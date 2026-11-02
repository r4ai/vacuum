use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct PythonAdapter;

const PYTHON_DIR_TARGETS: &[&str] = &["__pycache__", ".venv", "venv", "dist", "build", ".eggs"];
const PYTHON_CONTEXT_FILES: &[&str] = &[
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
];

fn has_python_context(ctx: &mut Context, dir: &Path) -> bool {
    PYTHON_CONTEXT_FILES.iter().any(|f| ctx.has_file(dir, f))
}

/// Whether Python bytecode (`__pycache__/`, `*.pyc`) in `dir` belongs to a
/// Python project: `dir` or one of its 3 nearest ancestors has a context
/// file, or `dir` holds a `.py` file.
fn has_bytecode_context(ctx: &mut Context, dir: &Path) -> bool {
    dir.ancestors().take(4).any(|p| has_python_context(ctx, p)) || ctx.has_extension(dir, "py")
}

impl Adapter for PythonAdapter {
    fn name(&self) -> &'static str {
        "python"
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        if !PYTHON_DIR_TARGETS.contains(&name) {
            return None;
        }
        let has_context = if name == "__pycache__" {
            has_bytecode_context(ctx, parent)
        } else {
            has_python_context(ctx, parent)
        };
        has_context.then(|| format!("Python build artifact ({name}/)"))
    }

    fn match_file(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name.ends_with(".pyc") && has_bytecode_context(ctx, parent))
            .then(|| "Python bytecode (.pyc)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn pycache_with_py_file_in_same_dir_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(
            targets
                .iter()
                .any(|t| t.path == dir.path().join("__pycache__"))
        );
    }

    #[test]
    fn pycache_with_pyproject_toml_in_ancestor_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        let subdir = dir.path().join("src").join("pkg");
        fs::create_dir_all(&subdir).unwrap();
        fs::create_dir(subdir.join("__pycache__")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == subdir.join("__pycache__")));
    }

    #[test]
    fn pycache_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(
            targets.iter().all(|t| !t.path.ends_with("__pycache__")),
            "Should not detect __pycache__ without Python context"
        );
    }

    #[test]
    fn pyc_file_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.py"), "").unwrap();
        let pyc = dir.path().join("main.pyc");
        fs::write(&pyc, b"\x00\x00\x00\x00").unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == pyc));
    }

    #[test]
    fn pyc_file_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        let pyc = dir.path().join("foo.pyc");
        fs::write(&pyc, b"x").unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().all(|t| t.path != pyc));
    }

    #[test]
    fn venv_with_requirements_txt_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask==2.0.0").unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join(".venv")));
    }

    #[test]
    fn plain_venv_with_requirements_txt_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "django").unwrap();
        fs::create_dir(dir.path().join("venv")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join("venv")));
    }

    #[test]
    fn venv_without_context_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().all(|t| t.path != dir.path().join(".venv")));
    }

    #[test]
    fn dist_with_setup_py_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("setup.py"), "from setuptools import setup").unwrap();
        fs::create_dir(dir.path().join("dist")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join("dist")));
    }

    #[test]
    fn build_with_setup_cfg_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("setup.cfg"), "[metadata]").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join("build")));
    }

    #[test]
    fn eggs_with_pyproject_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        fs::create_dir(dir.path().join(".eggs")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        assert!(targets.iter().any(|t| t.path == dir.path().join(".eggs")));
    }

    #[test]
    fn contents_of_detected_venv_not_listed_separately() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        let venv = dir.path().join(".venv");
        fs::create_dir_all(venv.join("lib")).unwrap();
        fs::create_dir(venv.join("__pycache__")).unwrap();

        let targets = scan_with(PythonAdapter, dir.path());
        let venv_count = targets
            .iter()
            .filter(|t| t.path == dir.path().join(".venv"))
            .count();
        let inner_pycache_count = targets
            .iter()
            .filter(|t| t.path == venv.join("__pycache__"))
            .count();
        assert_eq!(venv_count, 1);
        assert_eq!(inner_pycache_count, 0);
    }
}
