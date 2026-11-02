mod common;

use tempfile::TempDir;

fn node_project(root: &std::path::Path) {
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"");
}

#[test]
fn list_mode_prints_targets_without_deleting() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    node_project(root);

    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("[node]  node_modules"), "{stdout}");
    assert!(!stdout.contains("Dry run"), "{stdout}");
    assert!(root.join("node_modules").exists());
}

#[test]
fn list_mode_supports_json() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    node_project(root);

    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "list", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["targets"].as_array().unwrap().len(), 1);
    assert_eq!(value["result"]["dry_run"], true);
    assert!(root.join("node_modules").exists());
}

#[test]
fn safe_mode_without_terminal_suggests_list_mode() {
    let dir = TempDir::new().unwrap();
    let output = common::vacuum().arg(dir.path()).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--mode list"), "{stderr}");
}
