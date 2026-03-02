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
