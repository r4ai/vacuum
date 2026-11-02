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
    common::make_file(&root.join(".terraform").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("main.tf"), b"");
    make_targets(root);
    common::make_file(&root.join("terraform.tfstate"), b"keep");

    vacuum(root);

    assert!(
        !root.join(".terraform").exists(),
        ".terraform should be deleted"
    );
    assert!(root.join("main.tf").exists());
    assert!(
        root.join("terraform.tfstate").exists(),
        "terraform.tfstate should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join(".terraform").exists());
}

/// Local state must never be deleted with the provider cache.
#[test]
fn terraform_dir_holding_state_is_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("main.tf"), b"");
    common::make_file(&root.join(".terraform").join("terraform.tfstate"), b"{}");

    vacuum(root);

    assert!(root.join(".terraform").join("terraform.tfstate").exists());
}
