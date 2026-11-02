mod common;

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Two Node projects, `keep` and `drop`, each with `node_modules/`.
fn setup(root: &Path) {
    for name in ["keep", "drop"] {
        common::make_file(&root.join(name).join("package.json"), b"{}");
        common::make_file(&root.join(name).join("node_modules").join("a.js"), b"");
    }
}

fn vacuum(root: &Path, args: &[&str]) {
    let output = Command::new(common::vacuum_bin())
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
fn exclude_keeps_matching_paths() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(root, &["--exclude", "keep"]);

    assert!(root.join("keep").join("node_modules").exists());
    assert!(!root.join("drop").join("node_modules").exists());
}

#[test]
fn exclude_can_be_given_more_than_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    vacuum(
        root,
        &["--exclude", "keep", "--exclude", "drop/node_modules"],
    );

    assert!(root.join("keep").join("node_modules").exists());
    assert!(root.join("drop").join("node_modules").exists());
}

#[test]
fn vacuumignore_keeps_matching_paths() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);
    common::make_file(&root.join(".vacuumignore"), b"# comment\nkeep/\n");

    vacuum(root, &[]);

    assert!(root.join("keep").join("node_modules").exists());
    assert!(!root.join("drop").join("node_modules").exists());
}

#[test]
fn exclude_applies_to_gitignore_matches() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".gitignore"), b"*.log\n");
    common::make_file(&root.join("keep").join("a.log"), b"");
    common::make_file(&root.join("drop").join("b.log"), b"");

    vacuum(root, &["--gitignore", "--exclude", "keep"]);

    assert!(root.join("keep").join("a.log").exists());
    assert!(!root.join("drop").join("b.log").exists());
}

#[test]
fn invalid_exclude_pattern_is_an_error() {
    let dir = TempDir::new().unwrap();
    let output = Command::new(common::vacuum_bin())
        .arg(dir.path())
        .args(["--mode", "auto", "--exclude", "a/**b/[z-a]"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("--exclude"), "stderr: {stderr}");
}
