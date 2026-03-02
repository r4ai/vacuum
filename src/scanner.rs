use std::path::Path;
use std::collections::HashSet;

use crate::adapter::{Adapter, CleanTarget};
use crate::adapters::{
    CargoAdapter, GitignoreAdapter, GoAdapter, GradleAdapter, MavenAdapter, NodeAdapter,
    PythonAdapter,
};
use crate::cli::Cli;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{Adapter, CleanTarget};
    use std::path::{Path, PathBuf};

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
}
