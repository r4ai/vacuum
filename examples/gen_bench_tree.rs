//! Generate a large dummy tree for benchmarking vacuum.
//!
//! ```sh
//! cargo run --release --example gen_bench_tree -- <DIR> [PROJECTS]
//! ```
//!
//! `PROJECTS` (default: 200) sets the size. Every project is one of five
//! kinds (node, cargo, python, gradle, plain). Each kind holds source files
//! that must be kept and build artifacts that vacuum detects. The output
//! is deterministic, so runs on the same arguments are comparable.

use std::fs;
use std::io;
use std::path::Path;

/// Files per artifact directory; artifacts dominate the tree, like in reality.
const ARTIFACT_FILES: usize = 40;
/// Nested sub-directories per artifact directory.
const ARTIFACT_DIRS: usize = 10;
/// Source files per project.
const SOURCE_FILES: usize = 20;

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: gen_bench_tree <DIR> [PROJECTS]");
        std::process::exit(2);
    };
    let projects: usize = match args.next() {
        Some(n) => n
            .parse()
            .map_err(|e| io::Error::other(format!("PROJECTS: {e}")))?,
        None => 200,
    };

    let root = Path::new(&root);
    if root.exists() && root.read_dir()?.next().is_some() {
        eprintln!("{} is not empty", root.display());
        std::process::exit(2);
    }

    let mut targets = 0;
    for i in 0..projects {
        // Group projects into nested workspaces to make the tree deep.
        let dir = root
            .join(format!("group-{}", i % 10))
            .join(format!("team-{}", i % 7))
            .join(format!("project-{i}"));
        targets += generate_project(&dir, i % 5)?;
    }

    println!(
        "generated {projects} projects with {targets} targets in {}",
        root.display()
    );
    Ok(())
}

/// Create one project of `kind` in `dir` and return its target count.
fn generate_project(dir: &Path, kind: usize) -> io::Result<usize> {
    fill(&dir.join("src"), "file", "rs", SOURCE_FILES, 0)?;
    match kind {
        0 => {
            fs::write(dir.join("package.json"), "{}")?;
            fill(
                &dir.join("node_modules"),
                "index",
                "js",
                ARTIFACT_FILES,
                ARTIFACT_DIRS,
            )?;
            Ok(1)
        }
        1 => {
            fs::write(dir.join("Cargo.toml"), "[package]")?;
            fill(
                &dir.join("target"),
                "obj",
                "o",
                ARTIFACT_FILES,
                ARTIFACT_DIRS,
            )?;
            Ok(1)
        }
        2 => {
            fs::write(dir.join("pyproject.toml"), "[project]")?;
            fill(
                &dir.join(".venv"),
                "mod",
                "py",
                ARTIFACT_FILES,
                ARTIFACT_DIRS,
            )?;
            fill(&dir.join("pkg").join("__pycache__"), "mod", "pyc", 5, 0)?;
            Ok(2)
        }
        3 => {
            fs::write(dir.join("build.gradle"), "plugins {}")?;
            fill(
                &dir.join("build"),
                "Main",
                "class",
                ARTIFACT_FILES,
                ARTIFACT_DIRS,
            )?;
            Ok(1)
        }
        _ => {
            // A directory named like an artifact, but without context.
            fill(&dir.join("target"), "data", "txt", 5, 0)?;
            Ok(0)
        }
    }
}

/// Create `files` files in `dir`, plus `dirs` sub-directories holding
/// `files` files each.
fn fill(dir: &Path, stem: &str, ext: &str, files: usize, dirs: usize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for i in 0..files {
        fs::write(dir.join(format!("{stem}{i}.{ext}")), [b'x'; 256])?;
    }
    for d in 0..dirs {
        fill(&dir.join(format!("dir{d}")), stem, ext, files, 0)?;
    }
    Ok(())
}
