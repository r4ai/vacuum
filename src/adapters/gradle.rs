use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct GradleAdapter;

const BUILD_SCRIPTS: &[&str] = &["build.gradle", "build.gradle.kts"];
/// A multi-project root may hold only a settings script. Gradle keeps
/// its `.gradle/` there, but builds into `build/` only with a build script.
const SETTINGS_SCRIPTS: &[&str] = &["settings.gradle", "settings.gradle.kts"];

impl Adapter for GradleAdapter {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        // Check the name first so that unrelated directories cost no I/O.
        let context: &[&[&str]] = match name {
            ".gradle" => &[BUILD_SCRIPTS, SETTINGS_SCRIPTS],
            "build" => &[BUILD_SCRIPTS],
            _ => return None,
        };
        context
            .iter()
            .flat_map(|files| files.iter())
            .any(|f| ctx.has_file(parent, f))
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
