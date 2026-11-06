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

#[test]
fn stack_work_next_to_stack_yaml_is_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("stack.yaml"), b"");
    common::make_file(&root.join(".stack-work").join("x"), b"");

    vacuum(root);

    assert!(!root.join(".stack-work").exists());
}

#[test]
fn dist_newstyle_next_to_a_cabal_file_is_deleted() {
    for context in ["app.cabal", "cabal.project"] {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        common::make_file(&root.join(context), b"");
        common::make_file(&root.join("dist-newstyle").join("x"), b"");

        vacuum(root);

        assert!(!root.join("dist-newstyle").exists(), "next to {context}");
    }
}

/// Each directory needs its own tool's file.
#[test]
fn directories_with_the_other_tools_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("stack.yaml"), b"");
    common::make_file(&root.join("dist-newstyle").join("x"), b"");
    common::make_file(&root.join("lib").join("app.cabal"), b"");
    common::make_file(&root.join("lib").join(".stack-work").join("x"), b"");

    vacuum(root);

    assert!(root.join("dist-newstyle").exists());
    assert!(root.join("lib").join(".stack-work").exists());
}
