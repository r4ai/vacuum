use ignore::{DirEntry, WalkBuilder, WalkState};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::adapter::{Adapter, CleanTarget, Context, DirStats, WalkLimits, dir_stats};
use crate::adapters::{GitignoreAdapter, Kind, REGISTRY};
use crate::age::AgeIndex;
use crate::cli::Cli;
use crate::exclude::Excludes;
use crate::tracked::TrackedIndex;

/// Scanner settings extracted from CLI flags.
/// This is `Clone + Send + 'static`, enabling use across thread boundaries.
#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub no_size: bool,
    /// Only keep targets whose project is older than this (`--older-than`).
    pub older_than: Option<Duration>,
    /// Only keep targets of at least this many bytes (`--min-size`).
    pub min_size: Option<u64>,
    /// How far the walks go.
    pub limits: WalkLimits,
    /// Extra gitignore-style patterns to exclude (`--exclude`).
    pub exclude: Vec<String>,
    /// Whether to keep targets that contain files tracked by git.
    pub include_tracked: bool,
    /// Names of the enabled adapters (see [`REGISTRY`]).
    pub enabled: HashSet<&'static str>,
}

impl From<&Cli> for ScanConfig {
    fn from(cli: &Cli) -> Self {
        Self {
            no_size: cli.no_size,
            exclude: cli.exclude.clone(),
            older_than: cli.older_than,
            min_size: cli.min_size.map(|s| s.as_u64()),
            limits: WalkLimits {
                max_depth: cli.max_depth,
                one_file_system: cli.one_file_system,
            },
            include_tracked: cli.include_tracked,
            enabled: cli.adapters.enabled.clone(),
        }
    }
}

impl ScanConfig {
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }

    pub fn set_enabled(&mut self, name: &'static str, enabled: bool) {
        if enabled {
            self.enabled.insert(name);
        } else {
            self.enabled.remove(name);
        }
    }

    /// Whether at least one adapter is enabled.
    pub fn any_enabled(&self) -> bool {
        !self.enabled.is_empty()
    }

    /// The enabled entry-judging adapters, in priority order.
    fn entry_adapters(&self) -> Vec<&'static dyn Adapter> {
        REGISTRY
            .iter()
            .filter(|spec| self.is_enabled(spec.name))
            .filter_map(|spec| match spec.kind {
                Kind::Entry(adapter) => Some(adapter),
                Kind::Gitignore => None,
            })
            .collect()
    }

    fn gitignore_enabled(&self) -> bool {
        REGISTRY
            .iter()
            .any(|spec| matches!(spec.kind, Kind::Gitignore) && self.is_enabled(spec.name))
    }
}

/// Scan with the adapters enabled by `cli` and return the sorted targets.
pub fn scan_enabled(root: &Path, cli: &Cli) -> anyhow::Result<Vec<CleanTarget>> {
    let cfg = ScanConfig::from(cli);
    let mut all: Vec<CleanTarget> = Vec::new();
    scan_streaming(root, &cfg, &mut |t| all.push(t))?;
    all.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(all)
}

/// Scan with only `adapter` enabled and return the sorted targets.
/// Lets adapter unit tests exercise the production walk.
#[cfg(test)]
pub fn scan_with(adapter: impl Adapter, root: &Path) -> Vec<CleanTarget> {
    let mut all: Vec<CleanTarget> = Vec::new();
    let filters = Filters {
        excludes: Excludes::none(),
        include_tracked: true,
        older_than: None,
        min_size: None,
        limits: WalkLimits::default(),
    };
    walk(root, &[&adapter], false, false, &filters, &mut |t| {
        all.push(t)
    });
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all
}

/// Stream discovered targets one-by-one via a callback.
/// Callers that need to run this off the main thread should wrap it in
/// `std::thread::spawn` and pass a closure that sends through an `mpsc` channel.
pub fn scan_streaming(
    root: &Path,
    cfg: &ScanConfig,
    on_found: &mut dyn FnMut(CleanTarget),
) -> anyhow::Result<()> {
    let filters = Filters {
        excludes: Excludes::new(root, &cfg.exclude)?,
        include_tracked: cfg.include_tracked,
        older_than: cfg.older_than,
        min_size: cfg.min_size,
        limits: cfg.limits,
    };
    walk(
        root,
        &cfg.entry_adapters(),
        cfg.gitignore_enabled(),
        cfg.no_size,
        &filters,
        on_found,
    );
    Ok(())
}

/// Walk `root` once, asking `adapters` about each entry in order.
fn walk(
    root: &Path,
    adapters: &[&dyn Adapter],
    gitignore: bool,
    no_size: bool,
    filters: &Filters,
    on_found: &mut dyn FnMut(CleanTarget),
) {
    let mut out = Emitter {
        emitted: HashSet::new(),
        no_size,
        filters,
        tracked: TrackedIndex::default(),
        ages: AgeIndex::default(),
        on_found,
    };

    if adapters.is_empty() {
        // Only gitignore can be enabled — no need for a second walk.
        if gitignore {
            for target in scan_gitignore(root, filters) {
                out.emit(target, None);
            }
        }
        return;
    }

    // Gitignore matches are computed up front so that the walk below can
    // emit whichever of a gitignore match and an adapter match is the
    // ancestor. A target is never emitted together with its descendant.
    let mut ignored: HashMap<PathBuf, CleanTarget> = HashMap::new();
    if gitignore {
        ignored.extend(
            scan_gitignore(root, filters)
                .into_iter()
                .map(|t| (t.path.clone(), t)),
        );
    }

    // Worker threads judge entries; this thread reports what they find,
    // since `on_found` need not be `Send`.
    let ignored_paths: HashSet<PathBuf> = ignored.keys().cloned().collect();
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            // Directory sizes are computed as rayon tasks, so the walk never
            // waits for them. The scope ends once every size is known.
            rayon::scope(|sizes| {
                walk_parallel(root, adapters, &ignored_paths, no_size, filters, sizes, tx)
            })
        });
        for found in rx {
            match found {
                Found::Adapter(target, stats) => out.emit(target, Some(stats)),
                Found::Unsized(target) => out.emit(target, None),
                Found::Ignored(path) => {
                    if let Some(target) = ignored.remove(&path) {
                        out.emit(target, None);
                    }
                }
            }
        }
    });

    // Gitignore matches the walk did not reach. Those inside an emitted
    // target are already covered by it.
    let mut rest: Vec<CleanTarget> = ignored.into_values().collect();
    rest.sort_by(|a, b| a.path.cmp(&b.path));
    for target in rest {
        if !out.covers(&target.path) {
            out.emit(target, None);
        }
    }
}

/// A match found by a worker thread of [`walk_parallel`].
enum Found {
    /// An adapter matched the entry. Also carries what a walk of its tree
    /// learned (see [`dir_stats`]).
    Adapter(CleanTarget, DirStats),
    /// An adapter matched a directory whose size is not computed yet.
    /// Never sent through the channel.
    Unsized(CleanTarget),
    /// The entry is a gitignore match.
    Ignored(PathBuf),
}

/// Walk `root` on several threads and send each match through `tx`.
/// The walk does not descend into a matched directory.
fn walk_parallel(
    root: &Path,
    adapters: &[&dyn Adapter],
    ignored: &HashSet<PathBuf>,
    no_size: bool,
    filters: &Filters,
    sizes: &rayon::Scope<'_>,
    tx: mpsc::Sender<Found>,
) {
    let excludes = &filters.excludes;
    let one_file_system = filters.limits.one_file_system;
    WalkBuilder::new(root)
        .standard_filters(false)
        .follow_links(false)
        .max_depth(filters.limits.max_depth)
        .same_file_system(filters.limits.one_file_system)
        .build_parallel()
        .run(|| {
            let tx = tx.clone();
            let mut ctx = Context::default();
            Box::new(move |entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                let (state, found) = judge(&entry, adapters, ignored, excludes, &mut ctx);
                let found = match found {
                    Some(Found::Unsized(mut target)) => {
                        // Directory walks run as rayon tasks, off the walk.
                        let tx = tx.clone();
                        sizes.spawn(move |_| {
                            // One walk yields the size and nested repositories.
                            let stats = dir_stats(&target.path, !no_size, one_file_system);
                            target.size = stats.size;
                            let _ = tx.send(Found::Adapter(target, stats));
                        });
                        None
                    }
                    other => other,
                };
                if let Some(found) = found
                    && tx.send(found).is_err()
                {
                    // The receiver is gone; nobody needs more results.
                    return WalkState::Quit;
                }
                state
            })
        });
}

/// Judge one entry: adapters first, in priority order, then gitignore.
/// A matched directory is returned as [`Found::Unsized`].
fn judge(
    entry: &DirEntry,
    adapters: &[&dyn Adapter],
    ignored: &HashSet<PathBuf>,
    excludes: &Excludes,
    ctx: &mut Context,
) -> (WalkState, Option<Found>) {
    let path = entry.path();
    let (Some(parent), Some(file_type)) = (path.parent(), entry.file_type()) else {
        return (WalkState::Continue, None);
    };
    if excludes.is_excluded(path, file_type.is_dir()) {
        // Neither a target nor walked into.
        return (WalkState::Skip, None);
    }
    let name = entry.file_name().to_string_lossy();

    let matched = if file_type.is_dir() {
        adapters.iter().find_map(|a| {
            a.match_dir(&name, parent, ctx)
                .map(|description| (a.name(), description))
        })
    } else if file_type.is_file() {
        adapters.iter().find_map(|a| {
            a.match_file(&name, parent, ctx)
                .map(|description| (a.name(), description))
        })
    } else {
        None
    };
    let skip = if file_type.is_dir() {
        WalkState::Skip
    } else {
        WalkState::Continue
    };

    if let Some((adapter, description)) = matched {
        let mut target = CleanTarget {
            tracked: false,
            path: path.to_path_buf(),
            adapter,
            description,
            size: 0,
        };
        if file_type.is_dir() {
            (skip, Some(Found::Unsized(target)))
        } else {
            target.size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            // A file holds no repository.
            (skip, Some(Found::Adapter(target, DirStats::default())))
        }
    } else if ignored.contains(path) {
        (skip, Some(Found::Ignored(path.to_path_buf())))
    } else {
        (WalkState::Continue, None)
    }
}

/// Reports each target once and remembers what was reported.
/// Which found targets are reported.
struct Filters {
    excludes: Excludes,
    /// Whether to keep targets that contain files tracked by git.
    include_tracked: bool,
    /// Only keep targets whose project is older than this.
    older_than: Option<Duration>,
    /// Only keep targets of at least this many bytes.
    min_size: Option<u64>,
    /// How far the walks go.
    limits: WalkLimits,
}

struct Emitter<'a> {
    emitted: HashSet<PathBuf>,
    no_size: bool,
    filters: &'a Filters,
    tracked: TrackedIndex,
    ages: AgeIndex,
    on_found: &'a mut dyn FnMut(CleanTarget),
}

impl Emitter<'_> {
    /// Report `target`, unless it was reported already, is excluded, or
    /// contains tracked files that are not wanted. The checks here also
    /// cover gitignore matches.
    /// `stats` describe the target's tree, if already known.
    fn emit(&mut self, mut target: CleanTarget, stats: Option<DirStats>) {
        // A directory is deleted as a whole, so it must not hold an
        // excluded path either.
        let is_dir = target.path.is_dir();
        if self.filters.excludes.is_excluded(&target.path, is_dir)
            || (is_dir && self.filters.excludes.contains_excluded(&target.path))
        {
            return;
        }
        if self.filters.min_size.is_some_and(|min| target.size < min) {
            return;
        }
        if let Some(older_than) = self.filters.older_than
            && !self.ages.is_older_than(&target.path, older_than)
        {
            return;
        }
        let one_file_system = self.filters.limits.one_file_system;
        let stats = match stats {
            Some(stats) => stats,
            None if is_dir => dir_stats(&target.path, false, one_file_system),
            None => DirStats::default(),
        };
        // Deleting the target would delete files on another file system.
        if one_file_system && stats.crosses_mount {
            return;
        }
        target.tracked = self.tracked.is_tracked(&target.path, Some(&stats.repos));
        if target.tracked && !self.filters.include_tracked {
            return;
        }
        if self.emitted.insert(target.path.clone()) {
            if self.no_size {
                target.size = 0;
            }
            (self.on_found)(target);
        }
    }

    /// Whether `path` or one of its ancestors was already reported.
    fn covers(&self, path: &Path) -> bool {
        path.ancestors().any(|a| self.emitted.contains(a))
    }
}

/// Run the gitignore adapter, reporting a failure as a warning.
/// Excluded paths are not walked.
fn scan_gitignore(root: &Path, filters: &Filters) -> Vec<CleanTarget> {
    let excluded = |path: &Path, is_dir| filters.excludes.is_excluded(path, is_dir);
    GitignoreAdapter
        .scan_except(root, &excluded, filters.limits)
        .unwrap_or_else(|e| {
            eprintln!("Warning: adapter '{}' failed: {e}", GitignoreAdapter::NAME);
            Vec::new()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use std::fs;
    use tempfile::TempDir;

    fn enabled_cli() -> Cli {
        Cli::try_parse_from(["vacuum"]).unwrap()
    }

    #[test]
    fn scan_enabled_detects_mixed_targets_in_one_pass() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();
        fs::write(dir.path().join("module.pyc"), b"\x00").unwrap();

        let results = scan_enabled(dir.path(), &enabled_cli()).unwrap();
        let adapters: Vec<_> = results.iter().map(|t| t.adapter).collect();

        assert!(adapters.contains(&"node"));
        assert!(adapters.contains(&"cargo"));
        assert!(adapters.contains(&"python"));
        assert!(
            results
                .iter()
                .any(|t| t.path == dir.path().join("node_modules"))
        );
        assert!(results.iter().any(|t| t.path == dir.path().join("target")));
        assert!(results.iter().any(|t| t.path == dir.path().join(".venv")));
        assert!(
            results
                .iter()
                .any(|t| t.path == dir.path().join("module.pyc"))
        );
    }

    #[test]
    fn scan_enabled_priority_matches_adapter_order() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("setup.py"), "from setuptools import setup").unwrap();
        fs::write(dir.path().join("build.gradle"), "plugins {}").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let results = scan_enabled(dir.path(), &enabled_cli()).unwrap();
        let target = results
            .iter()
            .find(|t| t.path == dir.path().join("target"))
            .unwrap();
        let build = results
            .iter()
            .find(|t| t.path == dir.path().join("build"))
            .unwrap();

        assert_eq!(target.adapter, "cargo");
        assert_eq!(build.adapter, "python");
    }

    #[test]
    fn scan_enabled_respects_disabled_flags() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();

        let mut cli = enabled_cli();
        cli.adapters.enabled.remove("node");
        let results = scan_enabled(dir.path(), &cli).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn scan_enabled_with_no_size_sets_size_to_zero() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::create_dir_all(dir.path().join("node_modules").join("pkg")).unwrap();
        fs::write(
            dir.path().join("node_modules").join("pkg").join("index.js"),
            "x",
        )
        .unwrap();

        let mut cli = enabled_cli();
        cli.no_size = true;
        let results = scan_enabled(dir.path(), &cli).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, dir.path().join("node_modules"));
        assert_eq!(results[0].size, 0);
    }
}
