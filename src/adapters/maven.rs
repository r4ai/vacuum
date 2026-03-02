use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct MavenAdapter;

impl Adapter for MavenAdapter {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn description(&self) -> &str {
        "Maven build artifacts (target/)"
    }

    fn is_safe(&self) -> bool {
        true
    }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        let mut targets = Vec::new();
        let mut iter = WalkDir::new(root).follow_links(false).into_iter();

        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();

            if !entry.file_type().is_dir() {
                continue;
            }
            if entry.file_name() != "target" {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };
            if !parent.join("pom.xml").exists() {
                continue;
            }

            let size = compute_dir_size(path);
            targets.push(CleanTarget {
                path: path.to_path_buf(),
                adapter: self.name(),
                description: "Maven build artifacts (target/)".into(),
                size,
            });
            iter.skip_current_dir();
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_maven_project(root: &std::path::Path) {
        fs::write(root.join("pom.xml"), "<project/>").unwrap();
        fs::create_dir_all(root.join("target").join("classes")).unwrap();
        fs::write(root.join("target").join("classes").join("App.class"), b"cafebabe").unwrap();
    }

    #[test]
    fn no_target_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();
        let targets = MavenAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn target_without_pom_xml_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        let targets = MavenAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn target_with_pom_xml_detected() {
        let dir = TempDir::new().unwrap();
        make_maven_project(dir.path());
        let targets = MavenAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("target"));
        assert_eq!(targets[0].adapter, "maven");
    }

    #[test]
    fn multiple_maven_projects_each_detected() {
        let dir = TempDir::new().unwrap();
        let proj_a = dir.path().join("module_a");
        let proj_b = dir.path().join("module_b");
        fs::create_dir(&proj_a).unwrap();
        fs::create_dir(&proj_b).unwrap();
        make_maven_project(&proj_a);
        make_maven_project(&proj_b);

        let targets = MavenAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("target")));
        assert!(paths.contains(&&proj_b.join("target")));
    }
}
