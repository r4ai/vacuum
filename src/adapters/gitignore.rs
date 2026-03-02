use ignore::gitignore::GitignoreBuilder;
use std::path::Path;
use walkdir::WalkDir;

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
        let mut iter = WalkDir::new(root).follow_links(false).into_iter();

        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();

            // Skip the root itself
            if path == root {
                continue;
            }

            // Skip .git directory
            if path.components().any(|c| c.as_os_str() == ".git") {
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
                    iter.skip_current_dir();
                }
            }
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn no_gitignore_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("app.log"), "log").unwrap();
        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty());
    }

    #[test]
    fn gitignore_matching_file_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.path().join("app.log"), "log content").unwrap();
        fs::write(dir.path().join("app.py"), "# not ignored").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("app.log")));
        assert!(targets.iter().all(|t| t.path != dir.path().join("app.py")));
    }

    #[test]
    fn gitignore_matching_directory_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "dist/\n").unwrap();
        fs::create_dir_all(dir.path().join("dist").join("assets")).unwrap();
        fs::write(dir.path().join("dist").join("bundle.js"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(targets.iter().any(|t| t.path == dir.path().join("dist")));
    }

    #[test]
    fn git_dir_excluded_even_if_gitignore_would_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), ".git/\n").unwrap();
        fs::create_dir_all(dir.path().join(".git").join("objects")).unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(
            targets
                .iter()
                .all(|t| !t.path.starts_with(dir.path().join(".git")))
        );
    }

    #[test]
    fn contents_of_matched_dir_not_listed_separately() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        fs::create_dir_all(dir.path().join("build").join("output")).unwrap();
        fs::write(dir.path().join("build").join("app.js"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        let build_related: Vec<_> = targets
            .iter()
            .filter(|t| t.path.starts_with(dir.path().join("build")))
            .collect();
        assert_eq!(build_related.len(), 1);
        assert_eq!(build_related[0].path, dir.path().join("build"));
    }

    #[test]
    fn multiple_patterns_each_matched() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n*.tmp\n").unwrap();
        fs::write(dir.path().join("debug.log"), "").unwrap();
        fs::write(dir.path().join("temp.tmp"), "").unwrap();
        fs::write(dir.path().join("main.rs"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(
            targets
                .iter()
                .any(|t| t.path == dir.path().join("debug.log"))
        );
        assert!(
            targets
                .iter()
                .any(|t| t.path == dir.path().join("temp.tmp"))
        );
        assert!(targets.iter().all(|t| t.path != dir.path().join("main.rs")));
    }
}
