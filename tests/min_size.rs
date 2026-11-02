mod common;

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// `big` has a 2 MB `node_modules/`, `small` a tiny one.
fn setup(root: &Path) {
    for (name, size) in [("big", 2_000_000), ("small", 10)] {
        common::make_file(&root.join(name).join("package.json"), b"{}");
        common::make_file(
            &root.join(name).join("node_modules").join("a.bin"),
            &vec![0u8; size],
        );
    }
}

fn vacuum(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn min_size_keeps_small_targets() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    setup(root);

    let output = vacuum(root, &["--min-size", "1MB"]);

    assert!(output.status.success());
    assert!(!root.join("big").join("node_modules").exists());
    assert!(root.join("small").join("node_modules").exists());
}

#[test]
fn min_size_conflicts_with_no_size() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(dir.path(), &["--min-size", "1MB", "--no-size"]);
    assert!(!output.status.success());
}

#[test]
fn invalid_min_size_is_rejected() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(dir.path(), &["--min-size", "huge"]);
    assert!(!output.status.success());
}
