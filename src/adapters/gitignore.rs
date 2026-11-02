use ignore::gitignore::GitignoreBuilder;
use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct GitignoreAdapter;

/// Whether `dir` is the root of a Git working tree (main repo, worktree, or submodule).
fn is_git_root(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Whether any descendant of `dir` is the root of a Git working tree.
fn contains_git_root(dir: &Path) -> bool {
    WalkDir::new(dir)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name() == ".git")
}

impl Adapter for GitignoreAdapter {
    fn name(&self) -> &'static str {
        "gitignore"
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

            let is_dir = entry.file_type().is_dir();

            // A nested worktree, submodule, or repository has its own `.git`
            // (a file for worktrees and submodules, a directory otherwise).
            // Never delete it as a whole; apply its own `.gitignore` instead.
            if is_dir && is_git_root(path) {
                targets.extend(self.scan(path)?);
                iter.skip_current_dir();
                continue;
            }

            let relative = match path.strip_prefix(root) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Descendants of a directory we walked into (see below) must still
            // honor patterns that matched an ancestor, e.g. `.worktrees/`.
            let matched = gitignore.matched_path_or_any_parents(relative, is_dir);

            if matched.is_ignore() {
                // An ignored directory may hold a worktree or repository,
                // e.g. `.claude/worktrees/<name>`. Walk into it instead.
                if is_dir && contains_git_root(path) {
                    continue;
                }

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
    fn nested_worktree_not_matched_by_parent_gitignore() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), ".worktrees/\n").unwrap();
        let worktree = dir.path().join(".worktrees").join("feature");
        fs::create_dir_all(&worktree).unwrap();
        // A worktree has a `.git` file instead of a `.git` directory.
        fs::write(worktree.join(".git"), "gitdir: /somewhere\n").unwrap();
        fs::write(worktree.join("main.rs"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty(), "unexpected targets: {targets:?}");
    }

    #[test]
    fn nested_worktree_uses_its_own_gitignore() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), ".worktrees/\n").unwrap();
        let worktree = dir.path().join(".worktrees").join("feature");
        fs::create_dir_all(worktree.join("dist")).unwrap();
        fs::write(worktree.join(".git"), "gitdir: /somewhere\n").unwrap();
        fs::write(worktree.join(".gitignore"), "dist/\n").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        let paths: Vec<_> = targets.iter().map(|t| t.path.clone()).collect();
        assert_eq!(paths, vec![worktree.join("dist")]);
    }

    #[test]
    fn nested_repository_not_matched_by_parent_gitignore() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "vendor/\n").unwrap();
        let nested = dir.path().join("vendor").join("lib");
        fs::create_dir_all(nested.join(".git")).unwrap();
        fs::write(nested.join("lib.rs"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        assert!(targets.is_empty(), "unexpected targets: {targets:?}");
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
