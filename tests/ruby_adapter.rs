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
    common::make_file(&root.join("vendor").join("bundle").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("Gemfile"), b"");
    make_targets(root);
    common::make_file(&root.join(".bundle").join("config"), b"keep");
    common::make_file(&root.join("vendor").join("assets").join("app.js"), b"keep");
    common::make_file(&root.join("app").join("models").join("user.rb"), b"keep");

    vacuum(root);

    assert!(
        !root.join("vendor").join("bundle").exists(),
        "vendor/bundle should be deleted"
    );
    assert!(root.join("Gemfile").exists());
    assert!(
        root.join(".bundle").join("config").exists(),
        ".bundle/config should be kept"
    );
    assert!(
        root.join("vendor").join("assets").join("app.js").exists(),
        "vendor/assets/app.js should be kept"
    );
    assert!(
        root.join("app").join("models").join("user.rb").exists(),
        "app/models/user.rb should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join("vendor").join("bundle").exists());
}
