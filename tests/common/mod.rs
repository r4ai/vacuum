#![allow(dead_code)]

use std::fs;
use std::path::Path;

pub fn vacuum_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vacuum")
}

pub fn make_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

pub fn make_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}
