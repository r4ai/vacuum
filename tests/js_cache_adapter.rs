mod common;

use std::process::Command;
use tempfile::TempDir;

const CACHES: &[&str] = &[
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    ".angular",
];

#[test]
fn js_framework_caches_are_deleted() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    for cache in CACHES {
        common::make_file(&root.join(cache).join("x"), b"");
    }
    common::make_file(&root.join("src").join("app.js"), b"");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    for cache in CACHES {
        assert!(!root.join(cache).exists(), "{cache} should be deleted");
    }
    assert!(root.join("src").join("app.js").exists());
}

#[test]
fn js_caches_without_package_json_are_kept() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join(".next").join("x"), b"");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join(".next").exists());
}

#[test]
fn js_cache_flag_disables_the_adapter() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join(".next").join("x"), b"");

    let status = Command::new(common::vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--js-cache=false"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join(".next").exists());
}
