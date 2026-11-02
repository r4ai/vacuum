mod common;

use std::process::Command;
use tempfile::TempDir;

const CACHES: &[&str] = &[
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    "mypkg.egg-info",
];

#[test]
fn python_tool_caches_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("pyproject.toml"), b"[project]");
    for cache in CACHES {
        common::make_file(&root.join(cache).join("x"), b"");
    }
    common::make_file(&root.join("src").join("mypkg").join("__init__.py"), b"");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    for cache in CACHES {
        assert!(!root.join(cache).exists(), "{cache} should be deleted");
    }
    assert!(root.join("src").join("mypkg").join("__init__.py").exists());
}

#[test]
fn python_tool_caches_without_context_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".tox").join("x"), b"");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join(".tox").exists());
}
