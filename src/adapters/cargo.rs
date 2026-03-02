use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct CargoAdapter;

impl Adapter for CargoAdapter {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn description(&self) -> &str {
        "Rust/Cargo build artifacts (target/)"
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
            if entry.file_name() != "target" {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };
            if !parent.join("Cargo.toml").exists() {
                continue;
            }

            let size = compute_dir_size(path);
            targets.push(CleanTarget {
                path: path.to_path_buf(),
                adapter: self.name(),
                description: "Cargo build artifacts (target/)".into(),
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

    fn make_cargo_project(root: &std::path::Path) {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"",
        )
        .unwrap();
        fs::create_dir_all(root.join("target").join("debug")).unwrap();
        fs::write(root.join("target").join("debug").join("app"), b"binary").unwrap();
    }

    #[test]
    fn no_target_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let targets = CargoAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn target_without_cargo_toml_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        let targets = CargoAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn target_with_cargo_toml_detected() {
        let dir = TempDir::new().unwrap();
        make_cargo_project(dir.path());
        let targets = CargoAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("target"));
        assert_eq!(targets[0].adapter, "cargo");
    }

    #[test]
    fn nested_target_inside_target_not_rescanned() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let inner = dir.path().join("target").join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(inner.join("target")).unwrap();

        let targets = CargoAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("target"));
    }

    #[test]
    fn multiple_projects_each_detected() {
        let dir = TempDir::new().unwrap();
        let proj_a = dir.path().join("crate_a");
        let proj_b = dir.path().join("crate_b");
        fs::create_dir(&proj_a).unwrap();
        fs::create_dir(&proj_b).unwrap();
        make_cargo_project(&proj_a);
        make_cargo_project(&proj_b);

        let targets = CargoAdapter.scan(dir.path()).unwrap();
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("target")));
        assert!(paths.contains(&&proj_b.join("target")));
    }
}
