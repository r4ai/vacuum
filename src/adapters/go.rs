use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct GoAdapter;

impl Adapter for GoAdapter {
    fn name(&self) -> &'static str {
        "go"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name == "vendor" && ctx.has_file(parent, "go.mod"))
            .then(|| "Go vendor directory (vendor/)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    fn make_go_project(root: &std::path::Path) {
        fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.21").unwrap();
        fs::create_dir_all(root.join("vendor").join("github.com").join("pkg")).unwrap();
        fs::write(
            root.join("vendor")
                .join("github.com")
                .join("pkg")
                .join("lib.go"),
            "package pkg",
        )
        .unwrap();
    }

    #[test]
    fn no_vendor_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("go.mod"), "module example.com/app").unwrap();
        let targets = scan_with(GoAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn vendor_without_go_mod_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("vendor")).unwrap();
        let targets = scan_with(GoAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn vendor_with_go_mod_detected() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let targets = scan_with(GoAdapter, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("vendor"));
        assert_eq!(targets[0].adapter, "go");
    }

    #[test]
    fn multiple_go_projects_each_detected() {
        let dir = TempDir::new().unwrap();
        let proj_a = dir.path().join("service_a");
        let proj_b = dir.path().join("service_b");
        fs::create_dir(&proj_a).unwrap();
        fs::create_dir(&proj_b).unwrap();
        make_go_project(&proj_a);
        make_go_project(&proj_b);

        let targets = scan_with(GoAdapter, dir.path());
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("vendor")));
        assert!(paths.contains(&&proj_b.join("vendor")));
    }
}
