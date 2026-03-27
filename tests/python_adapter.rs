mod common;

use std::process::Command;
use tempfile::TempDir;

#[test]
fn auto_mode_deletes_python_pycache() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("main.py"), b"print('hello')");
    common::make_dir(&root.join("__pycache__"));
    common::make_file(
        &root.join("__pycache__").join("main.cpython-311.pyc"),
        b"\x00\x00\x00\x00",
    );

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("__pycache__").exists(),
        "__pycache__ should be deleted"
    );
    assert!(root.join("main.py").exists(), "main.py should be preserved");
}

#[test]
fn auto_mode_deletes_pyc_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("main.py"), b"print('hello')");
    common::make_file(&root.join("main.pyc"), b"\x00\x00\x00\x00");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !root.join("main.pyc").exists(),
        "main.pyc should be deleted"
    );
    assert!(root.join("main.py").exists(), "main.py should be preserved");
}

#[test]
fn auto_mode_deletes_python_venv() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("requirements.txt"), b"flask==2.0.0");
    common::make_dir(&root.join(".venv").join("lib"));
    common::make_file(&root.join(".venv").join("lib").join("python.so"), b"so");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join(".venv").exists(), ".venv should be deleted");
    assert!(
        root.join("requirements.txt").exists(),
        "requirements.txt should be preserved"
    );
}
