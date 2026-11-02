//! Detect candidates that contain files tracked by git.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Answers whether a path is, or contains, a file tracked by git.
///
/// Each repository is listed with `git ls-files` only once. Paths outside
/// any repository, or in a repository git cannot list, count as untracked.
#[derive(Default)]
pub struct TrackedIndex {
    /// Repository root → every tracked file and all of its ancestors,
    /// relative to the root. `None` when git could not list it.
    repos: HashMap<PathBuf, Option<HashSet<PathBuf>>>,
}

impl TrackedIndex {
    pub fn is_tracked(&mut self, path: &Path) -> bool {
        let Some(root) = repository_root(path) else {
            return false;
        };
        let Ok(relative) = path.strip_prefix(&root) else {
            return false;
        };
        let relative = relative.to_path_buf();
        self.repos
            .entry(root)
            .or_insert_with_key(|root| list_tracked(root))
            .as_ref()
            .is_some_and(|tracked| tracked.contains(&relative))
    }
}

/// The nearest ancestor of `path` (excluding `path` itself) with a `.git`
/// entry: a directory for a repository, a file for a worktree or submodule.
fn repository_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// Every tracked file in the repository at `root` plus all of its ancestor
/// directories, relative to `root`.
fn list_tracked(root: &Path) -> Option<HashSet<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--cached"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut tracked = HashSet::new();
    for file in output.stdout.split(|&b| b == 0).filter(|f| !f.is_empty()) {
        let file = PathBuf::from(String::from_utf8_lossy(file).into_owned());
        for ancestor in file.ancestors().filter(|a| !a.as_os_str().is_empty()) {
            if !tracked.insert(ancestor.to_path_buf()) {
                // Its ancestors are already in the set.
                break;
            }
        }
    }
    Some(tracked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn tracked_file_and_its_directories_are_tracked() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("vendor").join("lib")).unwrap();
        fs::write(root.join("vendor").join("lib").join("a.go"), "").unwrap();
        git(root, &["init", "-q"]);
        git(root, &["add", "."]);

        let mut index = TrackedIndex::default();
        assert!(index.is_tracked(&root.join("vendor")));
        assert!(index.is_tracked(&root.join("vendor").join("lib")));
        assert!(index.is_tracked(&root.join("vendor").join("lib").join("a.go")));
    }

    #[test]
    fn untracked_directory_is_not_tracked() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.go"), "").unwrap();
        git(root, &["init", "-q"]);
        git(root, &["add", "."]);
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules").join("a.js"), "").unwrap();

        let mut index = TrackedIndex::default();
        assert!(!index.is_tracked(&root.join("node_modules")));
    }

    #[test]
    fn path_outside_a_repository_is_not_tracked() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("vendor")).unwrap();

        let mut index = TrackedIndex::default();
        assert!(!index.is_tracked(&dir.path().join("vendor")));
    }
}
