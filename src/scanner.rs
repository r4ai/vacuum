use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::adapter::{Adapter, CleanTarget, compute_dir_size};
use crate::adapters::{
    CargoAdapter, GitignoreAdapter, GoAdapter, GradleAdapter, MavenAdapter, NodeAdapter,
    PythonAdapter,
};
use crate::cli::Cli;

/// Scanner settings extracted from CLI flags.
/// This is `Clone + Send + 'static`, enabling use across thread boundaries.
#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub no_size: bool,
    pub node: bool,
    pub cargo: bool,
    pub python: bool,
    pub go: bool,
    pub gradle: bool,
    pub maven: bool,
    pub gitignore: bool,
}

impl From<&Cli> for ScanConfig {
    fn from(cli: &Cli) -> Self {
        Self {
            no_size: cli.no_size,
            node: cli.node,
            cargo: cli.cargo,
            python: cli.python,
            go: cli.go,
            gradle: cli.gradle,
            maven: cli.maven,
            gitignore: cli.gitignore,
        }
    }
}

/// Build the list of enabled adapters from CLI flags.
pub fn build_adapters(cli: &Cli) -> Vec<Box<dyn Adapter>> {
    let mut adapters: Vec<Box<dyn Adapter>> = Vec::new();

    if cli.node {
        adapters.push(Box::new(NodeAdapter));
    }
    if cli.cargo {
        adapters.push(Box::new(CargoAdapter));
    }
    if cli.python {
        adapters.push(Box::new(PythonAdapter));
    }
    if cli.go {
        adapters.push(Box::new(GoAdapter));
    }
    if cli.gradle {
        adapters.push(Box::new(GradleAdapter));
    }
    if cli.maven {
        adapters.push(Box::new(MavenAdapter));
    }
    if cli.gitignore {
        adapters.push(Box::new(GitignoreAdapter));
    }

    adapters
}

/// Run all enabled adapters, collect and deduplicate results.
pub fn scan(root: &Path, adapters: &[Box<dyn Adapter>]) -> anyhow::Result<Vec<CleanTarget>> {
    let mut all: Vec<CleanTarget> = Vec::new();
    let mut seen_paths: HashSet<std::path::PathBuf> = HashSet::new();

    for adapter in adapters {
        match adapter.scan(root) {
            Ok(targets) => {
                for target in targets {
                    if seen_paths.insert(target.path.clone()) {
                        all.push(target);
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: adapter '{}' failed: {e}", adapter.name());
            }
        }
    }

    // Sort by path for deterministic, grouped display
    all.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(all)
}

const PYTHON_DIR_TARGETS: &[&str] = &["__pycache__", ".venv", "venv", "dist", "build", ".eggs"];
const PYTHON_CONTEXT_FILES: &[&str] = &[
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
];

fn has_python_context(dir: &Path) -> bool {
    PYTHON_CONTEXT_FILES.iter().any(|f| dir.join(f).exists())
}

fn has_gradle_context(dir: &Path) -> bool {
    dir.join("build.gradle").exists() || dir.join("build.gradle.kts").exists()
}

fn has_python_context_cached(cache: &mut HashMap<PathBuf, bool>, dir: &Path) -> bool {
    if let Some(has_context) = cache.get(dir) {
        return *has_context;
    }
    let has_context = has_python_context(dir);
    cache.insert(dir.to_path_buf(), has_context);
    has_context
}

fn has_gradle_context_cached(cache: &mut HashMap<PathBuf, bool>, dir: &Path) -> bool {
    if let Some(has_context) = cache.get(dir) {
        return *has_context;
    }
    let has_context = has_gradle_context(dir);
    cache.insert(dir.to_path_buf(), has_context);
    has_context
}

fn has_file_cached(cache: &mut HashMap<PathBuf, bool>, dir: &Path, name: &str) -> bool {
    if let Some(has_file) = cache.get(dir) {
        return *has_file;
    }
    let has_file = dir.join(name).exists();
    cache.insert(dir.to_path_buf(), has_file);
    has_file
}

fn has_python_source_file_cached(cache: &mut HashMap<PathBuf, bool>, dir: &Path) -> bool {
    if let Some(has_source) = cache.get(dir) {
        return *has_source;
    }
    let has_source = dir.read_dir().is_ok_and(|mut d| {
        d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|ext| ext == "py")))
    });
    cache.insert(dir.to_path_buf(), has_source);
    has_source
}

/// Scan enabled adapters with a single directory walk for core adapters.
/// Gitignore adapter is still executed separately due to matcher semantics.
pub fn scan_enabled(root: &Path, cli: &Cli) -> anyhow::Result<Vec<CleanTarget>> {
    let cfg = ScanConfig::from(cli);
    let mut all: Vec<CleanTarget> = Vec::new();
    scan_streaming(root, &cfg, &mut |t| all.push(t))?;
    all.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(all)
}

/// Stream discovered targets one-by-one via a callback.
/// Callers that need to run this off the main thread should wrap it in
/// `std::thread::spawn` and pass a closure that sends through an `mpsc` channel.
pub fn scan_streaming(
    root: &Path,
    cfg: &ScanConfig,
    on_found: &mut dyn FnMut(CleanTarget),
) -> anyhow::Result<()> {
    let core_enabled = cfg.node || cfg.cargo || cfg.python || cfg.go || cfg.gradle || cfg.maven;
    if !core_enabled && !cfg.gitignore {
        return Ok(());
    }
    if !core_enabled {
        // Only gitignore is enabled — fall back to the adapter scan path.
        let gitignore = GitignoreAdapter;
        match gitignore.scan(root) {
            Ok(targets) => {
                for mut target in targets {
                    if cfg.no_size {
                        target.size = 0;
                    }
                    on_found(target);
                }
            }
            Err(e) => eprintln!("Warning: adapter '{}' failed: {e}", gitignore.name()),
        }
        return Ok(());
    }

    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut iter = WalkDir::new(root).follow_links(false).into_iter();
    let mut node_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut cargo_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut go_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut maven_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut gradle_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut python_context_cache: HashMap<PathBuf, bool> = HashMap::new();
    let mut python_source_cache: HashMap<PathBuf, bool> = HashMap::new();

    while let Some(entry) = iter.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();

        if entry.file_type().is_dir() {
            let name = entry.file_name().to_string_lossy();
            let parent = match path.parent() {
                Some(parent) => parent,
                None => continue,
            };
            let mut matched: Option<(&'static str, String)> = None;

            if cfg.node
                && name == "node_modules"
                && has_file_cached(&mut node_context_cache, parent, "package.json")
            {
                matched = Some(("node", "Node.js dependencies (node_modules/)".into()));
            }
            if matched.is_none()
                && cfg.cargo
                && name == "target"
                && has_file_cached(&mut cargo_context_cache, parent, "Cargo.toml")
            {
                matched = Some(("cargo", "Cargo build artifacts (target/)".into()));
            }
            if matched.is_none() && cfg.python && PYTHON_DIR_TARGETS.contains(&name.as_ref()) {
                let has_context = if name == "__pycache__" {
                    let mut p = parent;
                    let mut found = false;
                    for _ in 0..4 {
                        if has_python_context_cached(&mut python_context_cache, p) {
                            found = true;
                            break;
                        }
                        match p.parent() {
                            Some(pp) => p = pp,
                            None => break,
                        }
                    }
                    found || has_python_source_file_cached(&mut python_source_cache, parent)
                } else {
                    has_python_context_cached(&mut python_context_cache, parent)
                };

                if has_context {
                    matched = Some(("python", format!("Python build artifact ({name}/)")));
                }
            }
            if matched.is_none()
                && cfg.go
                && name == "vendor"
                && has_file_cached(&mut go_context_cache, parent, "go.mod")
            {
                matched = Some(("go", "Go vendor directory (vendor/)".into()));
            }
            if matched.is_none()
                && cfg.gradle
                && (name == ".gradle" || name == "build")
                && has_gradle_context_cached(&mut gradle_context_cache, parent)
            {
                matched = Some(("gradle", format!("Gradle build artifact ({name}/)")));
            }
            if matched.is_none()
                && cfg.maven
                && name == "target"
                && has_file_cached(&mut maven_context_cache, parent, "pom.xml")
            {
                matched = Some(("maven", "Maven build artifacts (target/)".into()));
            }

            if let Some((adapter, description)) = matched {
                let path_buf = path.to_path_buf();
                if seen_paths.insert(path_buf.clone()) {
                    let size = if cfg.no_size {
                        0
                    } else {
                        compute_dir_size(path)
                    };
                    on_found(CleanTarget {
                        path: path_buf,
                        adapter,
                        description,
                        size,
                    });
                }
                iter.skip_current_dir();
            }
        } else if entry.file_type().is_file() && cfg.python {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(".pyc") {
                let path_buf = path.to_path_buf();
                if seen_paths.insert(path_buf.clone()) {
                    let size = if cfg.no_size {
                        0
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };
                    on_found(CleanTarget {
                        path: path_buf,
                        adapter: "python",
                        description: "Python bytecode (.pyc)".into(),
                        size,
                    });
                }
            }
        }
    }

    if cfg.gitignore {
        let gitignore = GitignoreAdapter;
        match gitignore.scan(root) {
            Ok(targets) => {
                for mut target in targets {
                    if seen_paths.insert(target.path.clone()) {
                        if cfg.no_size {
                            target.size = 0;
                        }
                        on_found(target);
                    }
                }
            }
            Err(e) => eprintln!("Warning: adapter '{}' failed: {e}", gitignore.name()),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{Adapter, CleanTarget};
    use crate::cli::Mode;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    struct FixedAdapter {
        name: &'static str,
        targets: Vec<PathBuf>,
    }

    impl Adapter for FixedAdapter {
        fn name(&self) -> &'static str {
            self.name
        }
        fn description(&self) -> &str {
            "test adapter"
        }
        fn is_safe(&self) -> bool {
            true
        }
        fn scan(&self, _root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
            Ok(self
                .targets
                .iter()
                .map(|p| CleanTarget {
                    path: p.clone(),
                    adapter: self.name,
                    description: "test".into(),
                    size: 0,
                })
                .collect())
        }
    }

    #[test]
    fn scan_returns_empty_for_no_adapters() {
        let adapters: Vec<Box<dyn Adapter>> = vec![];
        let results = scan(Path::new("/tmp"), &adapters).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn scan_deduplicates_by_path() {
        let path_a = PathBuf::from("/tmp/vac_test_dedup/a");
        let path_b = PathBuf::from("/tmp/vac_test_dedup/b");

        let adapter1 = Box::new(FixedAdapter {
            name: "first",
            targets: vec![path_a.clone()],
        }) as Box<dyn Adapter>;
        let adapter2 = Box::new(FixedAdapter {
            name: "second",
            targets: vec![path_a.clone(), path_b.clone()],
        }) as Box<dyn Adapter>;

        let results = scan(Path::new("/tmp"), &[adapter1, adapter2]).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results.iter().filter(|t| t.path == path_a).count(), 1);
        assert_eq!(results.iter().filter(|t| t.path == path_b).count(), 1);
    }

    #[test]
    fn scan_sorts_results_by_path() {
        let path_z = PathBuf::from("/tmp/vac_test_sort/z");
        let path_a = PathBuf::from("/tmp/vac_test_sort/a");
        let path_m = PathBuf::from("/tmp/vac_test_sort/m");

        let adapter = Box::new(FixedAdapter {
            name: "test",
            targets: vec![path_z.clone(), path_a.clone(), path_m.clone()],
        }) as Box<dyn Adapter>;

        let results = scan(Path::new("/tmp"), &[adapter]).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].path, path_a);
        assert_eq!(results[1].path, path_m);
        assert_eq!(results[2].path, path_z);
    }

    #[test]
    fn scan_first_occurrence_wins_on_dedup() {
        let path = PathBuf::from("/tmp/vac_test_first/x");

        let adapter1 = Box::new(FixedAdapter {
            name: "first",
            targets: vec![path.clone()],
        }) as Box<dyn Adapter>;
        let adapter2 = Box::new(FixedAdapter {
            name: "second",
            targets: vec![path.clone()],
        }) as Box<dyn Adapter>;

        let results = scan(Path::new("/tmp"), &[adapter1, adapter2]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].adapter, "first");
    }

    fn enabled_cli() -> Cli {
        Cli {
            path: PathBuf::from("."),
            mode: Mode::Safe,
            dry_run: false,
            no_size: false,
            generate_completions: None,
            node: true,
            cargo: true,
            python: true,
            go: true,
            gradle: true,
            maven: true,
            gitignore: false,
        }
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
        cli.node = false;
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
