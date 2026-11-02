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
    common::make_file(&root.join("vendor").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("composer.json"), b"");
    make_targets(root);
    common::make_file(&root.join("src").join("App.php"), b"keep");

    vacuum(root);

    assert!(!root.join("vendor").exists(), "vendor should be deleted");
    assert!(root.join("composer.json").exists());
    assert!(
        root.join("src").join("App.php").exists(),
        "src/App.php should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join("vendor").exists());
}

/// `vendor/` is also Go's; the adapter tells them apart by context.
#[test]
fn go_vendor_is_reported_as_go() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("go.mod"), b"module app\n");
    common::make_file(&root.join("vendor").join("x"), b"");

    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--dry-run", "--no-size"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[go]  vendor"), "{stdout}");
    assert!(!stdout.contains("[php]"), "{stdout}");
}
