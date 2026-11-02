mod common;

use tempfile::TempDir;

/// On Linux the trash lives in `$XDG_DATA_HOME/Trash`, so the test can
/// point it at a temporary directory.
#[cfg(target_os = "linux")]
#[test]
fn trash_moves_targets_to_the_trash() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    let data_home = dir.path().join("data");
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"x");
    common::make_dir(&data_home);

    let output = common::vacuum()
        .arg(&root)
        .args(["--mode", "auto", "--trash"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("node_modules").exists());
    let trashed = data_home
        .join("Trash")
        .join("files")
        .join("node_modules")
        .join("a.js");
    assert!(trashed.exists(), "node_modules should be in the trash");
}

#[test]
fn trash_with_dry_run_keeps_targets() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"x");

    let status = common::vacuum()
        .arg(root)
        .args(["--mode", "auto", "--trash", "--dry-run"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join("node_modules").exists());
}
