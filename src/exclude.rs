//! Paths the user excluded with `--exclude` or `.vacuumignore`.

use anyhow::Context as _;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};

/// The file under the scan root whose patterns are excluded.
pub const IGNORE_FILE: &str = ".vacuumignore";

/// Matches excluded paths. Patterns use the gitignore syntax and are
/// relative to the scan root.
pub struct Excludes {
    root: PathBuf,
    matcher: Gitignore,
}

impl Excludes {
    /// Exclude `patterns` and the patterns in `<root>/.vacuumignore`.
    pub fn new(root: &Path, patterns: &[String]) -> anyhow::Result<Self> {
        let mut builder = GitignoreBuilder::new(root);
        let file = root.join(IGNORE_FILE);
        if file.is_file()
            && let Some(e) = builder.add(&file)
        {
            return Err(e).with_context(|| format!("Invalid {}", file.display()));
        }
        for pattern in patterns {
            builder
                .add_line(None, pattern)
                .with_context(|| format!("Invalid --exclude pattern: {pattern}"))?;
        }
        Ok(Self {
            root: root.to_path_buf(),
            matcher: builder.build()?,
        })
    }

    /// Exclude nothing.
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            root: PathBuf::new(),
            matcher: Gitignore::empty(),
        }
    }

    /// Whether `path`, or one of its ancestors below the root, is excluded.
    pub fn is_excluded(&self, path: &Path, is_dir: bool) -> bool {
        match path.strip_prefix(&self.root) {
            Ok(relative) if !relative.as_os_str().is_empty() => self
                .matcher
                .matched_path_or_any_parents(relative, is_dir)
                .is_ignore(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn pattern_excludes_path_and_descendants() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let excludes = Excludes::new(root, &["work".into()]).unwrap();
        assert!(excludes.is_excluded(&root.join("work"), true));
        assert!(excludes.is_excluded(&root.join("work").join("node_modules"), true));
        assert!(!excludes.is_excluded(&root.join("other"), true));
    }

    #[test]
    fn vacuumignore_is_read() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join(IGNORE_FILE), "*.venv\n").unwrap();
        let excludes = Excludes::new(root, &[]).unwrap();
        assert!(excludes.is_excluded(&root.join("a").join("big.venv"), true));
    }

    #[test]
    fn root_is_never_excluded() {
        let dir = TempDir::new().unwrap();
        let excludes = Excludes::new(dir.path(), &["*".into()]).unwrap();
        assert!(!excludes.is_excluded(dir.path(), true));
    }
}
