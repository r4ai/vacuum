mod common;

use tempfile::TempDir;

#[test]
fn auto_dry_run_lists_each_target_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("x"), b"x");

    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "auto", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.matches("node_modules").count(),
        1,
        "listed more than once:\n{stdout}"
    );
    assert!(
        root.join("node_modules").exists(),
        "dry run deletes nothing"
    );
}
