use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct MavenAdapter;

impl Adapter for MavenAdapter {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name == "target" && ctx.has_file(parent, "pom.xml"))
            .then(|| "Maven build artifacts (target/)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    fn make_maven_project(root: &std::path::Path) {
        fs::write(root.join("pom.xml"), "<project/>").unwrap();
        fs::create_dir_all(root.join("target").join("classes")).unwrap();
        fs::write(
            root.join("target").join("classes").join("App.class"),
            b"cafebabe",
        )
        .unwrap();
    }

    #[test]
    fn no_target_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();
        let targets = scan_with(MavenAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn target_without_pom_xml_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        let targets = scan_with(MavenAdapter, dir.path());
        assert!(targets.is_empty());
    }

    #[test]
    fn target_with_pom_xml_detected() {
        let dir = TempDir::new().unwrap();
        make_maven_project(dir.path());
        let targets = scan_with(MavenAdapter, dir.path());
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

        let targets = scan_with(MavenAdapter, dir.path());
        assert_eq!(targets.len(), 2);
        let paths: Vec<_> = targets.iter().map(|t| &t.path).collect();
        assert!(paths.contains(&&proj_a.join("target")));
        assert!(paths.contains(&&proj_b.join("target")));
    }
}
