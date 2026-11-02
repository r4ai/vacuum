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
        // A candidate that is a repository itself (e.g. a `vendor/`
        // checkout) is tracked when its own index lists any file.
        if path.join(".git").exists() && self.tracked_files(path).is_some_and(|t| !t.is_empty()) {
            return true;
        }
        let Some(root) = repository_root(path) else {
            return false;
        };
        let Ok(relative) = path.strip_prefix(&root) else {
            return false;
        };
        let relative = relative.to_path_buf();
        self.tracked_files(&root)
            .is_some_and(|tracked| tracked.contains(&relative))
    }

    /// The cached listing of the repository at `root`.
    fn tracked_files(&mut self, root: &Path) -> Option<&HashSet<PathBuf>> {
        self.repos
            .entry(root.to_path_buf())
            .or_insert_with_key(|root| list_tracked(root))
            .as_ref()
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
        let file = path_from_bytes(file);
        for ancestor in file.ancestors().filter(|a| !a.as_os_str().is_empty()) {
            if !tracked.insert(ancestor.to_path_buf()) {
                // Its ancestors are already in the set.
                break;
            }
        }
    }
    Some(tracked)
}

/// A path from the raw bytes git prints. On Unix this keeps non-UTF-8
/// names intact; elsewhere git prints UTF-8.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
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
    fn candidate_that_is_a_repository_with_files_is_tracked() {
        let dir = TempDir::new().unwrap();
        let vendor = dir.path().join("vendor");
        fs::create_dir(&vendor).unwrap();
        fs::write(vendor.join("lib.go"), "").unwrap();
        git(&vendor, &["init", "-q"]);
        git(&vendor, &["add", "."]);

        let mut index = TrackedIndex::default();
        assert!(index.is_tracked(&vendor));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_is_matched() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let app = root.join(OsStr::from_bytes(b"app-\xff"));
        let Ok(()) = fs::create_dir_all(app.join("vendor")) else {
            // The filesystem rejects non-UTF-8 names (e.g. on macOS).
            return;
        };
        fs::write(app.join("vendor").join("lib.go"), "").unwrap();
        git(root, &["init", "-q"]);
        git(root, &["add", "."]);

        let mut index = TrackedIndex::default();
        assert!(index.is_tracked(&app.join("vendor")));
    }

    #[test]
    fn path_outside_a_repository_is_not_tracked() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("vendor")).unwrap();

        let mut index = TrackedIndex::default();
        assert!(!index.is_tracked(&dir.path().join("vendor")));
    }
}
