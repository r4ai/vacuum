use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct NodeAdapter;

impl Adapter for NodeAdapter {
    fn name(&self) -> &'static str {
        "node"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name == "node_modules" && ctx.has_file(parent, "package.json"))
            .then(|| "Node.js dependencies (node_modules/)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
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
        let targets = scan_with(NodeAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn node_modules_without_package_json_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        let targets = scan_with(NodeAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn node_modules_with_package_json_detected() {
        let dir = TempDir::new().unwrap();
        make_node_project(dir.path());
        let targets = scan_with(NodeAdapter, dir.path());
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

        let targets = scan_with(NodeAdapter, dir.path());
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

        let targets = scan_with(NodeAdapter, dir.path());
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("node_modules")));
        assert!(paths.contains(&&proj_b.join("node_modules")));
    }
}
