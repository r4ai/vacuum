use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single item that vacuum has identified as a candidate for deletion.
#[derive(Debug, Clone)]
pub struct CleanTarget {
    /// Absolute path to the file or directory.
    pub path: PathBuf,
    /// Name of the adapter that discovered this target (e.g. "cargo").
    pub adapter: &'static str,
    /// Human-readable label shown in the interactive list.
    pub description: String,
    /// Size in bytes (computed by the adapter during scan).
    pub size: u64,
}

/// Cached filesystem lookups shared by all adapters during one scan.
///
/// Many candidates share a parent directory, so each lookup is done once.
#[derive(Default)]
pub struct Context {
    files: HashMap<PathBuf, HashMap<&'static str, bool>>,
    extensions: HashMap<PathBuf, HashMap<&'static str, bool>>,
}

impl Context {
    /// Whether `dir` contains an entry named `name`.
    pub fn has_file(&mut self, dir: &Path, name: &'static str) -> bool {
        Self::cached(&mut self.files, dir, name, || dir.join(name).exists())
    }

    /// Whether `dir` contains an entry with the extension `ext` (e.g. `"py"`).
    pub fn has_extension(&mut self, dir: &Path, ext: &'static str) -> bool {
        Self::cached(&mut self.extensions, dir, ext, || {
            dir.read_dir().is_ok_and(|entries| {
                entries
                    .flatten()
                    .any(|entry| entry.path().extension().is_some_and(|e| e == ext))
            })
        })
    }

    fn cached(
        cache: &mut HashMap<PathBuf, HashMap<&'static str, bool>>,
        dir: &Path,
        key: &'static str,
        compute: impl FnOnce() -> bool,
    ) -> bool {
        if let Some(&hit) = cache.get(dir).and_then(|m| m.get(key)) {
            return hit;
        }
        let value = compute();
        cache
            .entry(dir.to_path_buf())
            .or_default()
            .insert(key, value);
        value
    }
}

/// The contract every cleanup adapter must satisfy.
///
/// An adapter only judges single entries. The scanner walks the tree once
/// and asks each enabled adapter in order; the first match wins and the
/// walk does not descend into a matched directory.
pub trait Adapter: Send + Sync {
    /// Short, lowercase identifier used in CLI flags (e.g. `"cargo"`).
    fn name(&self) -> &'static str;

    /// Whether the directory `name` inside `parent` is a target.
    /// Returns the human-readable description when it is.
    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String>;

    /// Whether the regular file `name` inside `parent` is a target.
    /// Returns the human-readable description when it is.
    fn match_file(&self, _name: &str, _parent: &Path, _ctx: &mut Context) -> Option<String> {
        None
    }
}

/// Compute the total byte count for a directory tree, best-effort.
/// Symlinks are not followed; errors on individual entries are skipped.
/// Sub-directories are measured in parallel on the rayon thread pool.
pub fn compute_dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let entries: Vec<_> = entries.flatten().collect();
    entries
        .par_iter()
        .map(|entry| match entry.file_type() {
            // `DirEntry::file_type` does not follow symlinks.
            Ok(t) if t.is_dir() => compute_dir_size(&entry.path()),
            Ok(t) if t.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn context_has_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let mut ctx = Context::default();
        assert!(ctx.has_file(dir.path(), "Cargo.toml"));
        assert!(!ctx.has_file(dir.path(), "package.json"));
    }

    #[test]
    fn context_has_extension() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.py"), "").unwrap();
        let mut ctx = Context::default();
        assert!(ctx.has_extension(dir.path(), "py"));
        assert!(!ctx.has_extension(dir.path(), "opam"));
    }

    #[cfg(unix)]
    #[test]
    fn compute_dir_size_does_not_follow_symlinks() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("big"), [0u8; 1000]).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        fs::write(dir.path().join("a"), b"abc").unwrap();
        assert_eq!(compute_dir_size(dir.path()), 3);
    }

    #[test]
    fn compute_dir_size_sums_deep_tree() {
        let dir = TempDir::new().unwrap();
        let mut expected = 0;
        for i in 0..5 {
            let sub = dir.path().join(format!("d{i}")).join("x").join("y");
            fs::create_dir_all(&sub).unwrap();
            for j in 0..10 {
                fs::write(sub.join(format!("f{j}")), vec![0u8; i + j]).unwrap();
                expected += (i + j) as u64;
            }
        }
        assert_eq!(compute_dir_size(dir.path()), expected);
    }

    #[test]
    fn compute_dir_size_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert_eq!(compute_dir_size(dir.path()), 0);
    }

    #[test]
    fn compute_dir_size_single_file() {
        let dir = TempDir::new().unwrap();
        let content = b"hello world";
        fs::write(dir.path().join("file.txt"), content).unwrap();
        assert_eq!(compute_dir_size(dir.path()), content.len() as u64);
    }

    #[test]
    fn compute_dir_size_sums_nested_files() {
        let dir = TempDir::new().unwrap();
        let a = b"aaa";
        let b_data = b"bb";
        fs::write(dir.path().join("a.txt"), a).unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("b.txt"), b_data).unwrap();
        assert_eq!(
            compute_dir_size(dir.path()),
            (a.len() + b_data.len()) as u64
        );
    }
}
