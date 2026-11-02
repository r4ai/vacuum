mod common;

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn vacuum(root: &Path) {
    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();
    assert!(status.success());
}

fn make_targets(root: &Path) {
    common::make_file(&root.join(".zig-cache").join("x"), b"");
    common::make_file(&root.join("zig-cache").join("x"), b"");
    common::make_file(&root.join("zig-out").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("build.zig"), b"");
    make_targets(root);
    common::make_file(&root.join("src").join("main.zig"), b"keep");

    vacuum(root);

    assert!(
        !root.join(".zig-cache").exists(),
        ".zig-cache should be deleted"
    );
    assert!(
        !root.join("zig-cache").exists(),
        "zig-cache should be deleted"
    );
    assert!(!root.join("zig-out").exists(), "zig-out should be deleted");
    assert!(root.join("build.zig").exists());
    assert!(
        root.join("src").join("main.zig").exists(),
        "src/main.zig should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join(".zig-cache").exists());
}
