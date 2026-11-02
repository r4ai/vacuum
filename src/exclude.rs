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
    /// Patterns relative to the scan root.
    matcher: Gitignore,
    /// `~/...` patterns, matched against paths relative to the home
    /// directory. They also cover a scan root inside an excluded path.
    home: Option<(PathBuf, Gitignore)>,
}

impl Excludes {
    /// Exclude `patterns` and the patterns in `<root>/.vacuumignore`.
    /// A pattern starting with `~/` is a path from the home directory.
    pub fn new(root: &Path, patterns: &[String]) -> anyhow::Result<Self> {
        let mut builder = GitignoreBuilder::new(root);
        let file = root.join(IGNORE_FILE);
        if file.is_file()
            && let Some(e) = builder.add(&file)
        {
            return Err(e).with_context(|| format!("Invalid {}", file.display()));
        }
        let home_dir = home_dir();
        let mut home_builder = home_dir.as_deref().map(GitignoreBuilder::new);
        for pattern in patterns {
            let (target, line) = match (pattern.strip_prefix("~/"), home_builder.as_mut()) {
                // Anchor it at the home directory.
                (Some(rest), Some(home)) => (home, format!("/{rest}")),
                // Without a home directory, it cannot match anything.
                (Some(_), None) => continue,
                (None, _) => (&mut builder, pattern.clone()),
            };
            target
                .add_line(None, &line)
                .with_context(|| format!("Invalid --exclude pattern: {pattern}"))?;
        }
        let home = match (home_dir, home_builder) {
            (Some(dir), Some(builder)) => Some((dir, builder.build()?)),
            _ => None,
        };
        Ok(Self {
            root: root.to_path_buf(),
            matcher: builder.build()?,
            home,
        })
    }

    /// Exclude nothing.
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            root: PathBuf::new(),
            matcher: Gitignore::empty(),
            home: None,
        }
    }

    /// Whether `path`, or one of its ancestors, is excluded. The scan root
    /// is excluded only by a `~/` pattern that covers it.
    pub fn is_excluded(&self, path: &Path, is_dir: bool) -> bool {
        let by_root = match path.strip_prefix(&self.root) {
            Ok(relative) if !relative.as_os_str().is_empty() => self
                .matcher
                .matched_path_or_any_parents(relative, is_dir)
                .is_ignore(),
            _ => false,
        };
        by_root
            || self.home.as_ref().is_some_and(|(home, matcher)| {
                path.strip_prefix(home).is_ok_and(|relative| {
                    !relative.as_os_str().is_empty()
                        && matcher
                            .matched_path_or_any_parents(relative, is_dir)
                            .is_ignore()
                })
            })
    }

    fn has_patterns(&self) -> bool {
        self.matcher.num_ignores() > 0
            || self
                .home
                .as_ref()
                .is_some_and(|(_, matcher)| matcher.num_ignores() > 0)
    }

    /// Whether any path strictly inside `dir` is excluded. Deleting `dir`
    /// as a whole would delete it too. Walks `dir` only when there are
    /// exclude patterns.
    pub fn contains_excluded(&self, dir: &Path) -> bool {
        if !self.has_patterns() {
            return false;
        }
        walkdir::WalkDir::new(dir)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| self.is_excluded(e.path(), e.file_type().is_dir()))
    }
}

/// The canonical home directory, if known.
fn home_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(home.canonicalize().unwrap_or(home))
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
    fn directory_holding_an_excluded_path_is_detected() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("node_modules").join("keep")).unwrap();
        fs::create_dir_all(root.join("other")).unwrap();
        let excludes = Excludes::new(root, &["node_modules/keep".into()]).unwrap();
        assert!(excludes.contains_excluded(&root.join("node_modules")));
        assert!(!excludes.contains_excluded(&root.join("other")));
    }

    #[test]
    fn root_is_never_excluded() {
        let dir = TempDir::new().unwrap();
        let excludes = Excludes::new(dir.path(), &["*".into()]).unwrap();
        assert!(!excludes.is_excluded(dir.path(), true));
    }
}
