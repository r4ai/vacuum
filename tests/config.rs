mod common;

use std::path::Path;
use std::process::Output;
use tempfile::TempDir;

/// Run vacuum on `root` with `config` as its config file.
fn vacuum(config_home: &Path, config: &str, root: &Path, args: &[&str]) -> Output {
    common::make_file(
        &config_home.join("vacuum").join("config.toml"),
        config.as_bytes(),
    );
    common::vacuum()
        .arg(root)
        .args(args)
        .env("XDG_CONFIG_HOME", config_home)
        .env("HOME", config_home)
        .output()
        .unwrap()
}

fn node_project(root: &Path) {
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(&root.join("node_modules").join("a.js"), b"");
}

#[test]
fn config_disables_an_adapter() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    node_project(&root);

    let output = vacuum(
        dir.path(),
        "[adapters]\nnode = false\n",
        &root,
        &["--mode", "auto"],
    );

    assert!(output.status.success());
    assert!(root.join("node_modules").exists());
}

#[test]
fn cli_flag_overrides_config() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    node_project(&root);

    let output = vacuum(
        dir.path(),
        "[adapters]\nnode = false\n",
        &root,
        &["--mode", "auto", "--node"],
    );

    assert!(output.status.success());
    assert!(!root.join("node_modules").exists());
}

#[test]
fn config_sets_the_default_mode() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    node_project(&root);

    // List mode never deletes and needs no terminal.
    let output = vacuum(dir.path(), "mode = \"list\"\n", &root, &[]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("node_modules"), "{stdout}");
    assert!(root.join("node_modules").exists());
}

#[test]
fn config_exclude_accepts_home_relative_paths() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("projects");
    node_project(&root.join("keep"));
    node_project(&root.join("drop"));

    let output = vacuum(
        dir.path(),
        "exclude = [\"~/projects/keep\"]\n",
        &root,
        &["--mode", "auto"],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.join("keep").join("node_modules").exists());
    assert!(!root.join("drop").join("node_modules").exists());
}

#[test]
fn unknown_config_key_is_an_error() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(
        dir.path(),
        "mdoe = \"list\"\n",
        dir.path(),
        &["--mode", "list"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("config.toml"), "{stderr}");
}

#[test]
fn unknown_adapter_in_config_is_an_error() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(
        dir.path(),
        "[adapters]\nnodee = false\n",
        dir.path(),
        &["--mode", "list"],
    );
    assert!(!output.status.success());
}

#[test]
fn home_relative_exclude_covers_a_root_inside_it() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("work").join("important").join("app");
    node_project(&project);

    let output = vacuum(
        dir.path(),
        "exclude = [\"~/work/important/**\"]\n",
        &project,
        &["--mode", "auto"],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("node_modules").exists());
}

#[test]
fn cli_min_size_wins_over_config_no_size() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    common::make_file(&root.join("package.json"), b"{}");
    common::make_file(
        &root.join("node_modules").join("a.bin"),
        &vec![0u8; 2_000_000],
    );

    let output = vacuum(
        dir.path(),
        "no_size = true\n",
        &root,
        &["--mode", "auto", "--min-size", "1MB"],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("node_modules").exists());
}

#[test]
fn config_with_both_no_size_and_min_size_is_an_error() {
    let dir = TempDir::new().unwrap();
    let output = vacuum(
        dir.path(),
        "no_size = true\nmin_size = \"1MB\"\n",
        dir.path(),
        &["--mode", "list"],
    );
    assert!(!output.status.success());
}
