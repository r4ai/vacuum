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

/// The contract every cleanup adapter must satisfy.
pub trait Adapter: Send + Sync {
    /// Short, lowercase identifier used in CLI flags (e.g. `"cargo"`).
    fn name(&self) -> &'static str;

    /// One-line human description shown in `--help` output.
    fn description(&self) -> &str;

    /// Returns `true` for adapters that are enabled by default (safe).
    /// Dangerous adapters return `false` (opt-in).
    fn is_safe(&self) -> bool;

    /// Walk `root` recursively and return all discovered clean targets.
    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>>;
}

/// Compute the total byte count for a directory tree, best-effort.
/// Symlinks are not followed; errors on individual entries are skipped.
pub fn compute_dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
