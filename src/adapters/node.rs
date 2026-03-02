use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct NodeAdapter;

impl Adapter for NodeAdapter {
    fn name(&self) -> &'static str {
        "node"
    }

    fn description(&self) -> &str {
        "Node.js/npm dependencies (node_modules/)"
    }

    fn is_safe(&self) -> bool {
        true
    }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        let mut targets = Vec::new();
        let mut skip_prefixes: Vec<std::path::PathBuf> = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if skip_prefixes.iter().any(|p| path.starts_with(p)) {
                continue;
            }

            if !entry.file_type().is_dir() {
                continue;
            }
            if entry.file_name() != "node_modules" {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };
            if !parent.join("package.json").exists() {
                continue;
            }

            let size = compute_dir_size(path);
            targets.push(CleanTarget {
                path: path.to_path_buf(),
                adapter: self.name(),
                description: "Node.js dependencies (node_modules/)".into(),
                size,
            });
            skip_prefixes.push(path.to_path_buf());
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_node_project(root: &std::path::Path) {
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("node_modules").join("dep")).unwrap();
        fs::write(root.join("node_modules").join("dep").join("index.js"), "").unwrap();
    }

    #[test]
    fn no_node_modules_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let targets = NodeAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn node_modules_without_package_json_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        let targets = NodeAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn node_modules_with_package_json_detected() {
        let dir = TempDir::new().unwrap();
        make_node_project(dir.path());
        let targets = NodeAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("node_modules"));
        assert_eq!(targets[0].adapter, "node");
    }

    #[test]
    fn nested_node_modules_inside_node_modules_not_rescanned() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let nested = dir.path().join("node_modules").join("inner");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("package.json"), "{}").unwrap();
        fs::create_dir(nested.join("node_modules")).unwrap();

        let targets = NodeAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("node_modules"));
    }

    #[test]
    fn multiple_projects_each_detected() {
        let dir = TempDir::new().unwrap();
        let proj_a = dir.path().join("project_a");
        let proj_b = dir.path().join("project_b");
        fs::create_dir(&proj_a).unwrap();
        fs::create_dir(&proj_b).unwrap();
        make_node_project(&proj_a);
        make_node_project(&proj_b);

        let targets = NodeAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("node_modules")));
        assert!(paths.contains(&&proj_b.join("node_modules")));
    }
}
