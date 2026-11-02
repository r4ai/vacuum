mod common;

use tempfile::TempDir;

#[test]
fn dry_run_shows_sizes_in_si_units() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    common::make_file(&root.join("package.json"), b"{}");
    common::make_dir(&root.join("node_modules"));
    common::make_file(
        &root.join("node_modules").join("blob"),
        &vec![0u8; 1_500_000],
    );

    let output = common::vacuum()
        .arg(root)
        .args(["--mode", "auto", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Would free 1.5 MB"),
        "size should be shown in SI units, got:\n{stdout}"
    );
}
