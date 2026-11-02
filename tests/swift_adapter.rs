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
    common::make_file(&root.join(".build").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("Package.swift"), b"");
    make_targets(root);
    common::make_file(
        &root.join("Sources").join("App").join("main.swift"),
        b"keep",
    );

    vacuum(root);

    assert!(!root.join(".build").exists(), ".build should be deleted");
    assert!(root.join("Package.swift").exists());
    assert!(
        root.join("Sources").join("App").join("main.swift").exists(),
        "Sources/App/main.swift should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join(".build").exists());
}
