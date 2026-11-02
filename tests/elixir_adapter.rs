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
    common::make_file(&root.join("_build").join("x"), b"");
    common::make_file(&root.join("deps").join("x"), b"");
}

#[test]
fn targets_next_to_the_context_file_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("mix.exs"), b"");
    make_targets(root);
    common::make_file(&root.join("lib").join("app.ex"), b"keep");

    vacuum(root);

    assert!(!root.join("_build").exists(), "_build should be deleted");
    assert!(!root.join("deps").exists(), "deps should be deleted");
    assert!(root.join("mix.exs").exists());
    assert!(
        root.join("lib").join("app.ex").exists(),
        "lib/app.ex should be kept"
    );
}

#[test]
fn targets_without_the_context_file_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_targets(root);

    vacuum(root);

    assert!(root.join("_build").exists());
}

/// `_build/` is also OCaml's; the adapter tells them apart by context.
#[test]
fn ocaml_build_is_reported_as_ocaml() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("dune-project"), b"");
    common::make_file(&root.join("_build").join("x"), b"");

    let output = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--dry-run", "--no-size"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[ocaml]  _build"), "{stdout}");
    assert!(!stdout.contains("[elixir]"), "{stdout}");
}
