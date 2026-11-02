mod common;

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

    let status = common::vacuum()
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

    let status = common::vacuum()
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

    let status = common::vacuum()
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

#[test]
fn auto_mode_keeps_pyc_without_python_context() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("other").join("foo.pyc"), b"x");

    let status = common::vacuum()
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        root.join("other").join("foo.pyc").exists(),
        "foo.pyc without Python context must be preserved"
    );
}

#[test]
fn auto_mode_deletes_pyc_with_context_in_ancestor() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("pyproject.toml"), b"[project]");
    let pyc = root.join("src").join("pkg").join("mod.pyc");
    common::make_file(&pyc, b"x");

    let status = common::vacuum()
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!pyc.exists(), "mod.pyc should be deleted");
}
