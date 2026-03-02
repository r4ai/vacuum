use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct GradleAdapter;

const GRADLE_TARGETS: &[&str] = &[".gradle", "build"];

fn has_gradle_context(dir: &Path) -> bool {
    dir.join("build.gradle").exists() || dir.join("build.gradle.kts").exists()
}

impl Adapter for GradleAdapter {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn description(&self) -> &str {
        "Gradle build artifacts (.gradle/, build/)"
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

            let name = entry.file_name().to_string_lossy();
            if !GRADLE_TARGETS.contains(&name.as_ref()) {
                continue;
            }

            let parent = match path.parent() {
                Some(p) => p,
                None => continue,
            };
            if !has_gradle_context(parent) {
                continue;
            }

            let size = compute_dir_size(path);
            targets.push(CleanTarget {
                path: path.to_path_buf(),
                adapter: self.name(),
                description: format!("Gradle build artifact ({name}/)"),
                size,
            });
            skip_prefixes.push(path.to_path_buf());
        }

        Ok(targets)
    }
}
