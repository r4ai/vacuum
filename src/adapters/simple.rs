//! Adapters that only need "a directory named X next to file Y".

use std::path::Path;

use crate::adapter::{Adapter, Context};

/// Matches directories named one of `dirs` whose parent holds one of
/// `context_files`. Most ecosystems fit this shape, so a new adapter is
/// usually one `SimpleAdapter` value in the registry.
pub struct SimpleAdapter {
    pub name: &'static str,
    /// Prefix of the description, e.g. `"JS framework cache"`.
    pub label: &'static str,
    pub dirs: &'static [&'static str],
    pub context_files: &'static [&'static str],
}

impl Adapter for SimpleAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        // Check the name first so that unrelated directories cost no I/O.
        (self.dirs.contains(&name) && self.context_files.iter().any(|f| ctx.has_file(parent, f)))
            .then(|| format!("{} ({name}/)", self.label))
    }
}

/// Next.js, Nuxt, SvelteKit, Turborepo, Parcel and Angular caches.
pub static JS_CACHE: SimpleAdapter = SimpleAdapter {
    name: "js-cache",
    label: "JS framework cache",
    dirs: &[
        ".next",
        ".nuxt",
        ".svelte-kit",
        ".turbo",
        ".parcel-cache",
        ".angular",
    ],
    context_files: &["package.json"],
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
        dirs: &["out"],
        context_files: &["a.toml", "b.toml"],
    };

    #[test]
    fn matches_a_listed_directory_next_to_any_context_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("b.toml"), "").unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        let targets = scan_with(TEST, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].description, "Test output (out/)");
    }

    #[test]
    fn ignores_a_directory_without_context() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("out")).unwrap();
        assert!(scan_with(TEST, dir.path()).is_empty());
    }

    #[test]
    fn ignores_other_directory_names() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.toml"), "").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();
        assert!(scan_with(TEST, dir.path()).is_empty());
    }
}
