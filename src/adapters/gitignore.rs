use std::path::Path;
use walkdir::WalkDir;
use ignore::gitignore::GitignoreBuilder;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct GitignoreAdapter;

impl Adapter for GitignoreAdapter {
    fn name(&self) -> &'static str {
        "gitignore"
    }

    fn description(&self) -> &str {
        "Files and directories matched by .gitignore (dangerous)"
    }

    fn is_safe(&self) -> bool {
        false
    }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        let root_gitignore = root.join(".gitignore");
        if !root_gitignore.exists() {
            return Ok(vec![]);
        }

        let mut builder = GitignoreBuilder::new(root);
        builder.add(&root_gitignore);
        let gitignore = builder.build()?;

        let mut targets = Vec::new();
        let mut skip_prefixes: Vec<std::path::PathBuf> = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip the root itself
            if path == root {
                continue;
            }

            // Skip .git directory
            if path
                .components()
                .any(|c| c.as_os_str() == ".git")
            {
                continue;
            }

            if skip_prefixes.iter().any(|p| path.starts_with(p)) {
                continue;
            }

            let relative = match path.strip_prefix(root) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let is_dir = entry.file_type().is_dir();
            let matched = gitignore.matched(relative, is_dir);

            if matched.is_ignore() {
                let size = if is_dir {
                    compute_dir_size(path)
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };
                targets.push(CleanTarget {
                    path: path.to_path_buf(),
                    adapter: self.name(),
                    description: format!("gitignore match: {}", relative.display()),
                    size,
                });
                if is_dir {
                    skip_prefixes.push(path.to_path_buf());
                }
            }
        }

        Ok(targets)
    }
}
