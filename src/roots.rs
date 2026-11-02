//! The directories given as `PATH` arguments.

use anyhow::Context as _;
use std::path::{Path, PathBuf};

/// The scan roots and the directory that shown paths are relative to.
pub struct Roots {
    /// Canonical, sorted directories; none is inside another.
    pub dirs: Vec<PathBuf>,
    /// The deepest common ancestor of `dirs`; the root itself when there
    /// is only one.
    pub base: PathBuf,
}

impl Roots {
    /// Resolve `paths`, each of which must be an accessible directory.
    pub fn resolve(paths: &[PathBuf]) -> anyhow::Result<Self> {
        let mut dirs = Vec::new();
        for path in paths {
            let dir = path
                .canonicalize()
                .with_context(|| format!("Cannot access path: {}", path.display()))?;
            if !dir.is_dir() {
                anyhow::bail!("{} is not a directory", dir.display());
            }
            dirs.push(dir);
        }
        let dirs = outermost(dirs);
        let base = common_ancestor(&dirs);
        Ok(Self { dirs, base })
    }
}

/// Sort `dirs` and drop every directory inside another one, since
/// scanning the outer directory already covers it.
fn outermost(mut dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    dirs.sort();
    dirs.dedup();
    let mut kept: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        // After sorting, an ancestor comes before its descendants.
        if !kept.iter().any(|outer| dir.starts_with(outer)) {
            kept.push(dir);
        }
    }
    kept
}

/// The deepest directory that contains every path in `dirs`.
fn common_ancestor(dirs: &[PathBuf]) -> PathBuf {
    let Some((first, rest)) = dirs.split_first() else {
        return PathBuf::new();
    };
    first
        .ancestors()
        .find(|candidate| rest.iter().all(|dir| dir.starts_with(candidate)))
        .unwrap_or(Path::new("/"))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn nested_directories_are_dropped() {
        assert_eq!(
            outermost(paths(&["/a/b", "/c", "/a", "/a/b/c", "/c"])),
            paths(&["/a", "/c"])
        );
    }

    #[test]
    fn sibling_with_common_prefix_is_kept() {
        assert_eq!(
            outermost(paths(&["/a/foo", "/a/foobar"])),
            paths(&["/a/foo", "/a/foobar"])
        );
    }

    #[test]
    fn common_ancestor_of_one_directory_is_itself() {
        assert_eq!(common_ancestor(&paths(&["/a/b"])), PathBuf::from("/a/b"));
    }

    #[test]
    fn common_ancestor_of_siblings_is_their_parent() {
        assert_eq!(
            common_ancestor(&paths(&["/home/u/work", "/home/u/oss"])),
            PathBuf::from("/home/u")
        );
    }
}
