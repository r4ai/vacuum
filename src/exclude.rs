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
    /// `~/...` patterns, anchored at the real directory they name. They
    /// also cover a scan root inside an excluded path.
    anchored: Vec<Anchored>,
}

/// A `~/...` pattern split into the real directory of its literal prefix
/// and the rest of the pattern.
///
/// The prefix is canonicalized like the scan roots, so `~/work/...` still
/// matches when `~/work` is a symlink to `/mnt/work`.
struct Anchored {
    base: PathBuf,
    /// The rest of the pattern, relative to `base`; `None` excludes `base`
    /// itself and everything below it.
    rest: Option<Gitignore>,
}

impl Anchored {
    fn new(home: &Path, pattern: &str) -> anyhow::Result<Self> {
        let is_glob = |part: &str| part.contains(['*', '?', '[', '{', '\\', '!']);
        let parts: Vec<&str> = pattern.split('/').filter(|p| !p.is_empty()).collect();
        let literal = parts.iter().take_while(|p| !is_glob(p)).count();
        let lexical = parts[..literal]
            .iter()
            .fold(home.to_path_buf(), |dir, part| dir.join(part));
        let base = lexical.canonicalize().unwrap_or(lexical);
        let rest = if literal == parts.len() {
            None
        } else {
            let mut builder = GitignoreBuilder::new(&base);
            builder.add_line(None, &format!("/{}", parts[literal..].join("/")))?;
            Some(builder.build()?)
        };
        Ok(Self { base, rest })
    }

    fn matches(&self, path: &Path, is_dir: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.base) else {
            return false;
        };
        match &self.rest {
            None => true,
            Some(rest) => {
                !relative.as_os_str().is_empty()
                    && rest
                        .matched_path_or_any_parents(relative, is_dir)
                        .is_ignore()
            }
        }
    }
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
        let home = home_dir();
        let mut anchored = Vec::new();
        for pattern in patterns {
            let invalid = || format!("Invalid --exclude pattern: {pattern}");
            match (pattern.strip_prefix("~/"), &home) {
                (Some(rest), Some(home)) => {
                    anchored.push(Anchored::new(home, rest).with_context(invalid)?)
                }
                // Without a home directory, it cannot match anything.
                (Some(_), None) => {}
                (None, _) => {
                    builder.add_line(None, pattern).with_context(invalid)?;
                }
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            matcher: builder.build()?,
            anchored,
        })
    }

    /// Exclude nothing.
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            root: PathBuf::new(),
            matcher: Gitignore::empty(),
            anchored: Vec::new(),
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
        by_root || self.anchored.iter().any(|a| a.matches(path, is_dir))
    }

    fn has_patterns(&self) -> bool {
        self.matcher.num_ignores() > 0 || !self.anchored.is_empty()
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
    // `dunce` avoids Windows' `\\?\C:\...` form, like the scan roots.
    Some(dunce::canonicalize(&home).unwrap_or(home))
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
