use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;
use walkdir::WalkDir;

use crate::adapter::{CleanTarget, EntryKind, WalkLimits, compute_dir_size, device_of, file_size};
use crate::skipped::SkippedPaths;

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

/// The ignore rules of one directory: `.git/info/exclude` (repository
/// root only), then its `.gitignore`. Later files take precedence.
fn rules_of(dir: &Path, repo_root: bool) -> anyhow::Result<Vec<Gitignore>> {
    let mut files = Vec::new();
    if repo_root {
        files.push(dir.join(".git").join("info").join("exclude"));
    }
    files.push(dir.join(".gitignore"));
    files
        .into_iter()
        .filter(|file| file.is_file())
        .map(|file| {
            let mut builder = GitignoreBuilder::new(dir);
            if let Some(e) = builder.add(&file) {
                return Err(e.into());
            }
            Ok(builder.build()?)
        })
        .collect()
}

/// The rules in force in a directory: those of each ancestor up to the
/// scan root, one level per directory, the deepest last. As in git, a
/// deeper file takes precedence over a shallower one.
struct Rules<'a>(&'a [Vec<Gitignore>]);

impl Rules<'_> {
    fn matched(&self, path: &Path, is_dir: bool) -> Match<()> {
        self.0
            .iter()
            .rev()
            .flat_map(|level| level.iter().rev())
            .map(
                |gitignore| match gitignore.matched_path_or_any_parents(path, is_dir) {
                    Match::None => Match::None,
                    Match::Ignore(_) => Match::Ignore(()),
                    Match::Whitelist(_) => Match::Whitelist(()),
                },
            )
            .find(|m| !m.is_none())
            .unwrap_or(Match::None)
    }

    fn has_whitelists(&self) -> bool {
        self.0.iter().flatten().any(|g| g.num_whitelists() > 0)
    }

    /// Whether any descendant of `dir` is re-included by a negation
    /// pattern (e.g. `!build/keep.txt`). Such a directory must not be
    /// deleted as a whole.
    fn whitelist_inside(&self, dir: &Path) -> bool {
        self.has_whitelists()
            && WalkDir::new(dir)
                .min_depth(1)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .any(|e| {
                    self.0
                        .iter()
                        .rev()
                        .flat_map(|level| level.iter().rev())
                        .map(|g| g.matched(e.path(), e.file_type().is_dir()))
                        .find(|m| !m.is_none())
                        .is_some_and(|m| m.is_whitelist())
                })
    }
}

/// Unlike the other adapters, gitignore matching depends on the patterns
/// of every ancestor and on nested repositories. It therefore runs its own
/// walk instead of judging single entries.
impl GitignoreAdapter {
    pub const NAME: &'static str = "gitignore";

    /// Walk `root` recursively and return all paths that its ignore rules
    /// match: every `.gitignore` in the tree, and `.git/info/exclude`.
    #[cfg(test)]
    pub fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        self.scan_except(
            root,
            &|_, _| false,
            WalkLimits::default(),
            &mut SkippedPaths::default(),
        )
    }

    /// Like `scan`, but neither walks nor returns paths for which
    /// `excluded(path, is_dir)` holds, nor paths beyond `limits`.
    /// Paths that cannot be read are recorded in `skipped`.
    pub fn scan_except(
        &self,
        root: &Path,
        excluded: &dyn Fn(&Path, bool) -> bool,
        limits: WalkLimits,
        skipped: &mut SkippedPaths,
    ) -> anyhow::Result<Vec<CleanTarget>> {
        // `levels[d]` holds the rules of the directory at depth `d` on the
        // path to the current entry.
        let mut levels: Vec<Vec<Gitignore>> = vec![rules_of(root, root.join(".git").is_dir())?];

        let mut targets = Vec::new();
        let mut walk = WalkDir::new(root)
            .follow_links(false)
            .same_file_system(limits.one_file_system);
        if let Some(max_depth) = limits.max_depth {
            walk = walk.max_depth(max_depth);
        }
        let mut iter = walk.into_iter();
        // The walk does not enter another file system, but still yields
        // the mount point itself; it must not become a target either.
        let root_device = if limits.one_file_system {
            device_of(root)
        } else {
            None
        };

        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    skipped.add_walkdir_error(error, root);
                    continue;
                }
            };
            let path = entry.path();

            // Skip the root itself
            if path == root {
                continue;
            }
            // The walk is depth-first: what is left above this entry's
            // depth belongs to its ancestors.
            levels.truncate(entry.depth());

            if root_device.is_some() && entry.file_type().is_dir() && device_of(path) != root_device
            {
                iter.skip_current_dir();
                continue;
            }

            if excluded(path, entry.file_type().is_dir()) {
                if entry.file_type().is_dir() {
                    iter.skip_current_dir();
                }
                continue;
            }

            // Skip .git directory
            if entry.file_name() == ".git" {
                if entry.file_type().is_dir() {
                    iter.skip_current_dir();
                }
                continue;
            }

            let is_dir = entry.file_type().is_dir();

            // A nested worktree, submodule, or repository has its own `.git`
            // (a file for worktrees and submodules, a directory otherwise).
            // Never delete it as a whole; apply its own `.gitignore` instead.
            if is_dir && is_git_root(path) {
                let depth = entry.depth();
                targets.extend(self.scan_except(path, excluded, limits.below(depth), skipped)?);
                iter.skip_current_dir();
                continue;
            }

            let relative = match path.strip_prefix(root) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Descendants of a directory we walked into (see below) must still
            // honor patterns that matched an ancestor, e.g. `.worktrees/`.
            let rules = Rules(&levels);
            let matched = rules.matched(path, is_dir);

            if !matched.is_ignore() {
                if is_dir {
                    // A directory's own `.gitignore` applies to what is inside.
                    levels.push(rules_of(path, false)?);
                }
                continue;
            }

            // An ignored directory may hold a worktree or repository,
            // e.g. `.claude/worktrees/<name>`. Walk into it instead.
            if is_dir && contains_git_root(path) {
                levels.push(rules_of(path, false)?);
                continue;
            }

            // Likewise, walk into an ignored directory that holds a path
            // re-included by a negation pattern, and keep that path.
            if is_dir && rules.whitelist_inside(path) {
                levels.push(rules_of(path, false)?);
                continue;
            }

            let size = if is_dir {
                compute_dir_size(path)
            } else {
                entry.metadata().map(|m| file_size(&m)).unwrap_or(0)
            };
            targets.push(CleanTarget {
                tracked: false,
                kind: EntryKind::of(entry.file_type()),
                path: path.to_path_buf(),
                adapter: Self::NAME,
                description: format!("gitignore match: {}", relative.display()),
                size,
            });
            if is_dir {
                iter.skip_current_dir();
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

    fn paths(root: &Path) -> Vec<std::path::PathBuf> {
        let mut paths: Vec<_> = GitignoreAdapter
            .scan(root)
            .unwrap()
            .into_iter()
            .map(|t| t.path.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn nested_gitignore_applies_to_its_directory() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("web").join("out")).unwrap();
        fs::create_dir_all(root.join("out")).unwrap();
        fs::write(root.join("web").join(".gitignore"), "out/\n").unwrap();

        assert_eq!(paths(root), [Path::new("web/out")]);
    }

    #[test]
    fn info_exclude_is_applied() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git").join("info")).unwrap();
        fs::write(root.join(".git").join("info").join("exclude"), "scratch/\n").unwrap();
        fs::create_dir_all(root.join("scratch")).unwrap();

        assert_eq!(paths(root), [Path::new("scratch")]);
    }

    #[test]
    fn deeper_gitignore_overrides_shallower() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub").join(".gitignore"), "!keep.log\n").unwrap();
        fs::write(root.join("sub").join("keep.log"), "").unwrap();
        fs::write(root.join("sub").join("a.log"), "").unwrap();
        fs::write(root.join("b.log"), "").unwrap();

        assert_eq!(paths(root), [Path::new("b.log"), Path::new("sub/a.log")]);
    }

    #[test]
    fn nested_patterns_do_not_apply_to_siblings() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a").join(".gitignore"), "*.tmp\n").unwrap();
        fs::write(root.join("a").join("x.tmp"), "").unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("b").join("y.tmp"), "").unwrap();

        assert_eq!(paths(root), [Path::new("a/x.tmp")]);
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
    fn whitelisted_file_inside_ignored_dir_is_kept() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "build/\n!build/keep.txt\n").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build").join("keep.txt"), "").unwrap();
        fs::write(dir.path().join("build").join("out.o"), "").unwrap();

        let targets = GitignoreAdapter.scan(dir.path()).unwrap();
        let paths: Vec<_> = targets.iter().map(|t| t.path.clone()).collect();
        assert_eq!(paths, vec![dir.path().join("build").join("out.o")]);
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
