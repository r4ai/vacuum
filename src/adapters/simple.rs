//! Adapters defined as data: "a directory named X next to file Y".

use std::path::Path;

use crate::adapter::{Adapter, Context};

/// One way an adapter finds targets.
///
/// Patterns:
/// - `dirs`: `name`, `*.suffix` (e.g. `*.egg-info`), or `parent/name`
///   (e.g. `vendor/bundle`), which also requires the parent's name.
/// - `context`: `file` or `*.ext` (e.g. `*.csproj`), looked up in the
///   directory that holds the pattern (the parent of `name`, or of
///   `parent` for `parent/name`).
pub struct Rule {
    pub dirs: &'static [&'static str],
    pub context: &'static [&'static str],
    /// A target holding any of these entries is not a target, e.g.
    /// precious state that cannot be regenerated.
    pub unless_inside: &'static [&'static str],
}

impl Rule {
    /// The project directory of a match, or `None` when `name` in
    /// `parent` does not match this rule.
    fn project<'p>(&self, name: &str, parent: &'p Path) -> Option<&'p Path> {
        self.dirs
            .iter()
            .find_map(|pattern| match pattern.split_once('/') {
                Some((outer, inner)) => (inner == name
                    && parent.file_name().is_some_and(|n| n == outer))
                .then(|| parent.parent())
                .flatten(),
                None => match pattern.strip_prefix('*') {
                    Some(suffix) => {
                        (name.ends_with(suffix) && name.len() > suffix.len()).then_some(parent)
                    }
                    None => (*pattern == name).then_some(parent),
                },
            })
    }

    fn has_context(&self, project: &Path, ctx: &mut Context) -> bool {
        self.context
            .iter()
            .any(|pattern| match pattern.strip_prefix("*.") {
                Some(ext) => ctx.has_extension(project, ext),
                None => ctx.has_file(project, pattern),
            })
    }
}

/// An adapter made of [`Rule`]s. Most ecosystems fit this shape, so a new
/// adapter is usually one `SimpleAdapter` value in the registry.
pub struct SimpleAdapter {
    pub name: &'static str,
    /// Prefix of the description, e.g. `"JS framework cache"`.
    pub label: &'static str,
    pub rules: &'static [Rule],
}

impl Adapter for SimpleAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        self.rules.iter().find_map(|rule| {
            // The name check comes first so that unrelated directories
            // cost no I/O.
            let project = rule.project(name, parent)?;
            let target = parent.join(name);
            (rule.has_context(project, ctx)
                && !rule.unless_inside.iter().any(|f| target.join(f).exists()))
            .then(|| format!("{} ({name}/)", self.label))
        })
    }
}

/// Next.js, Nuxt, SvelteKit, Turborepo, Parcel and Angular caches.
pub static JS_CACHE: SimpleAdapter = SimpleAdapter {
    name: "js-cache",
    label: "JS framework cache",
    rules: &[Rule {
        dirs: &[
            ".next",
            ".nuxt",
            ".svelte-kit",
            ".turbo",
            ".parcel-cache",
            ".angular",
        ],
        context: &["package.json"],
        unless_inside: &[],
    }],
};

/// pytest, mypy, Ruff, tox and nox caches, and setuptools metadata.
pub static PYTHON_CACHE: SimpleAdapter = SimpleAdapter {
    name: "python-cache",
    label: "Python tool cache",
    rules: &[Rule {
        dirs: &[
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".tox",
            ".nox",
            "*.egg-info",
        ],
        context: &[
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "requirements.txt",
        ],
        unless_inside: &[],
    }],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    const TEST: SimpleAdapter = SimpleAdapter {
        name: "test",
        label: "Test output",
        rules: &[
            Rule {
                dirs: &["out", "*.gen", "vendor/bundle"],
                context: &["a.toml", "*.proj"],
                unless_inside: &["keep.state"],
            },
            Rule {
                dirs: &["cache"],
                context: &["b.toml"],
                unless_inside: &[],
            },
        ],
    };

    fn found(root: &Path) -> Vec<String> {
        scan_with(TEST, root)
            .iter()
            .map(|t| t.path.strip_prefix(root).unwrap().display().to_string())
            .collect()
    }

    #[test]
    fn matches_a_listed_directory_next_to_a_context_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        let targets = scan_with(TEST, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].description, "Test output (out/)");
    }

    #[test]
    fn context_can_be_an_extension() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("app.proj"), "").unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        assert_eq!(found(dir.path()), vec!["out"]);
    }

    #[test]
    fn dir_can_be_a_suffix() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir(dir.path().join("x.gen")).unwrap();
        fs::create_dir(dir.path().join(".gen")).unwrap();
        assert_eq!(found(dir.path()), vec!["x.gen"]);
    }

    #[test]
    fn dir_can_be_nested_one_level() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir_all(dir.path().join("vendor").join("bundle")).unwrap();
        fs::create_dir_all(dir.path().join("other").join("bundle")).unwrap();
        let expected = Path::new("vendor").join("bundle").display().to_string();
        assert_eq!(found(dir.path()), vec![expected]);
    }

    #[test]
    fn each_rule_has_its_own_context() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir(dir.path().join("cache")).unwrap();
        assert!(found(dir.path()).is_empty());
        fs::write(dir.path().join("b.toml"), "").unwrap();
        assert_eq!(found(dir.path()), vec!["cache"]);
    }

    #[test]
    fn target_holding_a_protected_entry_is_kept() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        fs::write(dir.path().join("out").join("keep.state"), "").unwrap();
        assert!(found(dir.path()).is_empty());
    }

    #[test]
    fn ignores_a_directory_without_context() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        assert!(found(dir.path()).is_empty());
    }
}
