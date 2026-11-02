use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, Context, compute_dir_size};
use crate::adapters::{GitignoreAdapter, Kind, REGISTRY};
use crate::cli::Cli;

/// Scanner settings extracted from CLI flags.
/// This is `Clone + Send + 'static`, enabling use across thread boundaries.
#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub no_size: bool,
    /// Names of the enabled adapters (see [`REGISTRY`]).
    pub enabled: HashSet<&'static str>,
}

impl From<&Cli> for ScanConfig {
    fn from(cli: &Cli) -> Self {
        Self {
            no_size: cli.no_size,
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
    walk(root, &[&adapter], false, false, &mut |t| all.push(t));
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
    walk(
        root,
        &cfg.entry_adapters(),
        cfg.gitignore_enabled(),
        cfg.no_size,
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
    on_found: &mut dyn FnMut(CleanTarget),
) {
    let mut out = Emitter {
        emitted: HashSet::new(),
        no_size,
        on_found,
    };

    if adapters.is_empty() {
        // Only gitignore can be enabled — no need for a second walk.
        if gitignore {
            for target in scan_gitignore(root) {
                out.emit(target);
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
            scan_gitignore(root)
                .into_iter()
                .map(|t| (t.path.clone(), t)),
        );
    }

    let mut ctx = Context::default();
    let mut iter = WalkDir::new(root).follow_links(false).into_iter();

    while let Some(entry) = iter.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let Some(parent) = path.parent() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy();
        let file_type = entry.file_type();

        if file_type.is_dir() {
            let matched = adapters.iter().find_map(|a| {
                a.match_dir(&name, parent, &mut ctx)
                    .map(|description| (a.name(), description))
            });
            if let Some((adapter, description)) = matched {
                let size = if no_size { 0 } else { compute_dir_size(path) };
                out.emit(CleanTarget {
                    path: path.to_path_buf(),
                    adapter,
                    description,
                    size,
                });
                iter.skip_current_dir();
            } else if let Some(target) = ignored.remove(path) {
                out.emit(target);
                iter.skip_current_dir();
            }
        } else if file_type.is_file() {
            let matched = adapters.iter().find_map(|a| {
                a.match_file(&name, parent, &mut ctx)
                    .map(|description| (a.name(), description))
            });
            if let Some((adapter, description)) = matched {
                let size = if no_size {
                    0
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };
                out.emit(CleanTarget {
                    path: path.to_path_buf(),
                    adapter,
                    description,
                    size,
                });
            } else if let Some(target) = ignored.remove(path) {
                out.emit(target);
            }
        }
    }

    // Gitignore matches the walk did not reach. Those inside an emitted
    // target are already covered by it.
    let mut rest: Vec<CleanTarget> = ignored.into_values().collect();
    rest.sort_by(|a, b| a.path.cmp(&b.path));
    for target in rest {
        if !out.covers(&target.path) {
            out.emit(target);
        }
    }
}

/// Reports each target once and remembers what was reported.
struct Emitter<'a> {
    emitted: HashSet<PathBuf>,
    no_size: bool,
    on_found: &'a mut dyn FnMut(CleanTarget),
}

impl Emitter<'_> {
    fn emit(&mut self, mut target: CleanTarget) {
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
fn scan_gitignore(root: &Path) -> Vec<CleanTarget> {
    GitignoreAdapter.scan(root).unwrap_or_else(|e| {
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
