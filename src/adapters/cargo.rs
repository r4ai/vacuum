use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct CargoAdapter;

impl Adapter for CargoAdapter {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name == "target" && ctx.has_file(parent, "Cargo.toml"))
            .then(|| "Cargo build artifacts (target/)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
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
        let targets = scan_with(CargoAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn target_without_cargo_toml_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        let targets = scan_with(CargoAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn target_with_cargo_toml_detected() {
        let dir = TempDir::new().unwrap();
        make_cargo_project(dir.path());
        let targets = scan_with(CargoAdapter, dir.path());
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

        let targets = scan_with(CargoAdapter, dir.path());
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

        let targets = scan_with(CargoAdapter, dir.path());
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("target")));
        assert!(paths.contains(&&proj_b.join("target")));
    }
}
