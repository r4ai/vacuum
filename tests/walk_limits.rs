mod common;

use std::path::Path;
use tempfile::TempDir;

/// `node_modules/` at depth 1 (root project) and depth 2 (`sub` project).
fn setup(root: &Path) {
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"");
    common::make_file(&root.join("sub").join("package.json"), b"{}");
    common::make_file(&root.join("sub").join("node_modules").join("a.js"), b"");
}

fn vacuum(root: &Path, args: &[&str]) {
    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "auto"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn max_depth_limits_how_deep_targets_are_found() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(root, &["--max-depth", "1"]);

    assert!(!root.join("node_modules").exists());
    assert!(root.join("sub").join("node_modules").exists());
}

#[test]
fn max_depth_applies_to_gitignore_matches() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"*.log\n");
    common::make_file(&root.join("a.log"), b"");
    common::make_file(&root.join("sub").join("b.log"), b"");

    vacuum(root, &["--gitignore", "--max-depth", "1"]);

    assert!(!root.join("a.log").exists());
    assert!(root.join("sub").join("b.log").exists());
}

/// Crossing a mount point cannot be set up in a test; this only checks
/// that the same file system is still cleaned.
#[test]
fn one_file_system_still_cleans_the_same_file_system() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(root, &["--one-file-system"]);

    assert!(!root.join("node_modules").exists());
    assert!(!root.join("sub").join("node_modules").exists());
}
