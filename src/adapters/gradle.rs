use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct GradleAdapter;

const GRADLE_TARGETS: &[&str] = &[".gradle", "build"];
const GRADLE_CONTEXT_FILES: &[&str] = &["build.gradle", "build.gradle.kts"];

impl Adapter for GradleAdapter {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        let has_context = GRADLE_CONTEXT_FILES.iter().any(|f| ctx.has_file(parent, f));
        (GRADLE_TARGETS.contains(&name) && has_context)
            .then(|| format!("Gradle build artifact ({name}/)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn gradle_dir_with_build_gradle_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.gradle"), "plugins {}").unwrap();
        fs::create_dir(dir.path().join(".gradle")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join(".gradle")));
        assert_eq!(targets[0].adapter, "gradle");
    }

    #[test]
    fn build_dir_with_build_gradle_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.gradle"), "plugins {}").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join("build")));
    }

    #[test]
    fn gradle_dir_with_build_gradle_kts_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.gradle.kts"), "plugins {}").unwrap();
        fs::create_dir(dir.path().join(".gradle")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join(".gradle")));
    }

    #[test]
    fn both_gradle_and_build_dirs_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.gradle"), "plugins {}").unwrap();
        fs::create_dir(dir.path().join(".gradle")).unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn gradle_dir_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".gradle")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert!(targets.iter().all(|t| t.path != dir.path().join(".gradle")));
    }

    #[test]
    fn build_dir_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let targets = scan_with(GradleAdapter, dir.path());
        assert!(targets.iter().all(|t| t.path != dir.path().join("build")));
    }
}
