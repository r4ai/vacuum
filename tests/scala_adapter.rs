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
    common::make_file(&root.join("target").join("x"), b"");
    common::make_file(&root.join("project").join("target").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("build.sbt"), b"");
    make_targets(root);
    common::make_file(&root.join("project").join("build.properties"), b"keep");
    common::make_file(
        &root
            .join("src")
            .join("main")
            .join("scala")
            .join("App.scala"),
        b"keep",
    );

    vacuum(root);

    assert!(!root.join("target").exists(), "target should be deleted");
    assert!(
        !root.join("project").join("target").exists(),
        "project/target should be deleted"
    );
    assert!(root.join("build.sbt").exists());
    assert!(
        root.join("project").join("build.properties").exists(),
        "project/build.properties should be kept"
    );
    assert!(
        root.join("src")
            .join("main")
            .join("scala")
            .join("App.scala")
            .exists(),
        "src/main/scala/App.scala should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join("target").exists());
}
