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
