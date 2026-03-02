use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn vacuum_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vacuum")
}

fn make_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn make_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

// ── Node adapter ──────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("package.json"), b"{}");
    make_dir(&root.join("node_modules").join("dep"));
    make_file(&root.join("node_modules").join("dep").join("index.js"), b"// code");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("node_modules").exists(), "node_modules should be deleted");
    assert!(root.join("package.json").exists(), "package.json should be preserved");
}

#[test]
fn dry_run_does_not_delete_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("package.json"), b"{}");
    make_dir(&root.join("node_modules").join("dep"));
    make_file(&root.join("node_modules").join("dep").join("index.js"), b"// code");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--dry-run"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join("node_modules").exists(), "node_modules should survive dry-run");
}

#[test]
fn node_adapter_disabled_skips_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("package.json"), b"{}");
    make_dir(&root.join("node_modules").join("dep"));

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto", "--node=false"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(root.join("node_modules").exists(), "node_modules should be preserved when adapter is disabled");
}

// ── Cargo adapter ─────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_cargo_target() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(
        &root.join("Cargo.toml"),
        b"[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"",
    );
    make_dir(&root.join("target").join("debug"));
    make_file(&root.join("target").join("debug").join("app"), b"binary");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("target").exists(), "target/ should be deleted");
    assert!(root.join("Cargo.toml").exists(), "Cargo.toml should be preserved");
}

// ── Python adapter ────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_python_pycache() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("main.py"), b"print('hello')");
    make_dir(&root.join("__pycache__"));
    make_file(&root.join("__pycache__").join("main.cpython-311.pyc"), b"\x00\x00\x00\x00");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("__pycache__").exists(), "__pycache__ should be deleted");
    assert!(root.join("main.py").exists(), "main.py should be preserved");
}

#[test]
fn auto_mode_deletes_pyc_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("main.py"), b"print('hello')");
    make_file(&root.join("main.pyc"), b"\x00\x00\x00\x00");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("main.pyc").exists(), "main.pyc should be deleted");
    assert!(root.join("main.py").exists(), "main.py should be preserved");
}

#[test]
fn auto_mode_deletes_python_venv() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("requirements.txt"), b"flask==2.0.0");
    make_dir(&root.join(".venv").join("lib"));
    make_file(&root.join(".venv").join("lib").join("python.so"), b"so");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join(".venv").exists(), ".venv should be deleted");
    assert!(root.join("requirements.txt").exists(), "requirements.txt should be preserved");
}

// ── Go adapter ────────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_go_vendor() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("go.mod"), b"module example.com/app\n\ngo 1.21");
    make_dir(&root.join("vendor").join("github.com").join("pkg"));
    make_file(
        &root.join("vendor").join("github.com").join("pkg").join("lib.go"),
        b"package pkg",
    );

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("vendor").exists(), "vendor/ should be deleted");
    assert!(root.join("go.mod").exists(), "go.mod should be preserved");
}

// ── Gradle adapter ────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_gradle_artifacts() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("build.gradle"), b"plugins { id 'java' }");
    make_dir(&root.join(".gradle").join("8.0"));
    make_dir(&root.join("build").join("classes"));
    make_file(&root.join("build").join("classes").join("App.class"), b"cafebabe");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join(".gradle").exists(), ".gradle/ should be deleted");
    assert!(!root.join("build").exists(), "build/ should be deleted");
    assert!(root.join("build.gradle").exists(), "build.gradle should be preserved");
}

// ── Maven adapter ─────────────────────────────────────────────────────────────

#[test]
fn auto_mode_deletes_maven_target() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join("pom.xml"), b"<project/>");
    make_dir(&root.join("target").join("classes"));
    make_file(&root.join("target").join("classes").join("App.class"), b"cafebabe");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("target").exists(), "target/ should be deleted");
    assert!(root.join("pom.xml").exists(), "pom.xml should be preserved");
}

// ── No targets ────────────────────────────────────────────────────────────────

#[test]
fn no_targets_exits_cleanly() {
    let dir = TempDir::new().unwrap();
    make_file(&dir.path().join("hello.txt"), b"hello");

    let status = Command::new(vacuum_bin())
        .arg(dir.path())
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
}

// ── All adapters disabled ─────────────────────────────────────────────────────

#[test]
fn all_adapters_disabled_exits_cleanly() {
    let dir = TempDir::new().unwrap();
    make_file(&dir.path().join("package.json"), b"{}");
    make_dir(&dir.path().join("node_modules"));

    let output = Command::new(vacuum_bin())
        .arg(dir.path())
        .args([
            "--mode",
            "auto",
            "--node=false",
            "--cargo=false",
            "--python=false",
            "--go=false",
            "--gradle=false",
            "--maven=false",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("All adapters are disabled"),
        "Expected message about disabled adapters, got: {stdout}"
    );
    assert!(dir.path().join("node_modules").exists(), "node_modules should be untouched");
}

// ── Gitignore adapter ─────────────────────────────────────────────────────────

#[test]
fn gitignore_adapter_deletes_matched_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    make_file(&root.join(".gitignore"), b"*.log\n");
    make_file(&root.join("app.log"), b"log content");
    make_file(&root.join("app.py"), b"# not ignored");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args([
            "--mode",
            "auto",
            "--gitignore",
            "--node=false",
            "--cargo=false",
            "--python=false",
            "--go=false",
            "--gradle=false",
            "--maven=false",
        ])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("app.log").exists(), "app.log should be deleted (matches *.log)");
    assert!(root.join("app.py").exists(), "app.py should be preserved");
}

// ── Mixed project ─────────────────────────────────────────────────────────────

#[test]
fn auto_mode_handles_mixed_project() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Node sub-project
    make_file(&root.join("frontend").join("package.json"), b"{}");
    make_dir(&root.join("frontend").join("node_modules").join("dep"));
    make_file(
        &root.join("frontend").join("node_modules").join("dep").join("index.js"),
        b"",
    );

    // Rust sub-project
    make_file(
        &root.join("backend").join("Cargo.toml"),
        b"[package]\nname=\"backend\"\nversion=\"0.1.0\"\nedition=\"2021\"",
    );
    make_dir(&root.join("backend").join("target").join("debug"));
    make_file(&root.join("backend").join("target").join("debug").join("app"), b"binary");

    let status = Command::new(vacuum_bin())
        .arg(root)
        .args(["--mode", "auto"])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!root.join("frontend").join("node_modules").exists(), "node_modules should be deleted");
    assert!(!root.join("backend").join("target").exists(), "target/ should be deleted");
    assert!(root.join("frontend").join("package.json").exists(), "package.json should be preserved");
    assert!(root.join("backend").join("Cargo.toml").exists(), "Cargo.toml should be preserved");
}

// ── Invalid path ──────────────────────────────────────────────────────────────

#[test]
fn invalid_path_exits_with_error() {
    let status = Command::new(vacuum_bin())
        .arg("/nonexistent/path/that/does/not/exist/vacuum_test")
        .status()
        .unwrap();

    assert!(!status.success(), "Should fail for a non-existent path");
}

#[test]
fn file_path_instead_of_dir_exits_with_error() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("file.txt");
    make_file(&file, b"content");

    let status = Command::new(vacuum_bin()).arg(&file).status().unwrap();

    assert!(!status.success(), "Should fail when path is a file, not a directory");
}
