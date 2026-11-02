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
    /// The project directory of each pattern that matches `name` in
    /// `parent`. Several can match: `target` and `project/target` both
    /// match a `target` directory, with different projects.
    fn projects<'p>(&self, name: &str, parent: &'p Path) -> impl Iterator<Item = &'p Path> {
        self.dirs
            .iter()
            .filter_map(move |pattern| match pattern.split_once('/') {
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
        let matched = self.rules.iter().any(|rule| {
            // The name check comes first so that unrelated directories
            // cost no I/O.
            rule.projects(name, parent)
                .any(|project| rule.has_context(project, ctx))
                && !rule
                    .unless_inside
                    .iter()
                    .any(|f| parent.join(name).join(f).exists())
        });
        matched.then(|| format!("{} ({name}/)", self.label))
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

/// .NET build output next to a project file.
pub static DOTNET: SimpleAdapter = SimpleAdapter {
    name: "dotnet",
    label: ".NET build output",
    rules: &[Rule {
        dirs: &["bin", "obj"],
        context: &["*.csproj", "*.fsproj", "*.vbproj"],
        unless_inside: &[],
    }],
};

/// Swift Package Manager build output.
pub static SWIFT: SimpleAdapter = SimpleAdapter {
    name: "swift",
    label: "SwiftPM build output",
    rules: &[Rule {
        dirs: &[".build"],
        context: &["Package.swift"],
        unless_inside: &[],
    }],
};

/// Stack and Cabal build output.
pub static HASKELL: SimpleAdapter = SimpleAdapter {
    name: "haskell",
    label: "Haskell build output",
    rules: &[
        Rule {
            dirs: &[".stack-work"],
            context: &["stack.yaml"],
            unless_inside: &[],
        },
        Rule {
            dirs: &["dist-newstyle"],
            context: &["*.cabal", "cabal.project"],
            unless_inside: &[],
        },
    ],
};

/// Mix build output and dependencies.
pub static ELIXIR: SimpleAdapter = SimpleAdapter {
    name: "elixir",
    label: "Elixir build artifact",
    rules: &[Rule {
        dirs: &["_build", "deps"],
        context: &["mix.exs"],
        unless_inside: &[],
    }],
};

/// Zig build cache and output.
pub static ZIG: SimpleAdapter = SimpleAdapter {
    name: "zig",
    label: "Zig build artifact",
    rules: &[Rule {
        dirs: &[".zig-cache", "zig-cache", "zig-out"],
        context: &["build.zig"],
        unless_inside: &[],
    }],
};

/// Dart and Flutter tool state and build output.
pub static DART: SimpleAdapter = SimpleAdapter {
    name: "dart",
    label: "Dart build artifact",
    rules: &[Rule {
        dirs: &[".dart_tool", "build"],
        context: &["pubspec.yaml"],
        unless_inside: &[],
    }],
};

/// Composer dependencies.
pub static PHP: SimpleAdapter = SimpleAdapter {
    name: "php",
    label: "Composer dependencies",
    rules: &[Rule {
        dirs: &["vendor"],
        context: &["composer.json"],
        unless_inside: &[],
    }],
};

/// Bundler gems installed into the project. `.bundle/` is kept: its `config` holds user settings.
pub static RUBY: SimpleAdapter = SimpleAdapter {
    name: "ruby",
    label: "Bundler gems",
    rules: &[Rule {
        dirs: &["vendor/bundle"],
        context: &["Gemfile"],
        unless_inside: &[],
    }],
};

/// sbt build output, including the build definition's own.
pub static SCALA: SimpleAdapter = SimpleAdapter {
    name: "scala",
    label: "sbt build output",
    rules: &[Rule {
        dirs: &["target", "project/target"],
        context: &["build.sbt"],
        unless_inside: &[],
    }],
};

/// Terraform providers and modules. Kept when it holds state.
pub static TERRAFORM: SimpleAdapter = SimpleAdapter {
    name: "terraform",
    label: "Terraform cache",
    rules: &[Rule {
        dirs: &[".terraform"],
        context: &["*.tf"],
        unless_inside: &["terraform.tfstate"],
    }],
};

/// Elm build cache.
pub static ELM: SimpleAdapter = SimpleAdapter {
    name: "elm",
    label: "Elm build cache",
    rules: &[Rule {
        dirs: &["elm-stuff"],
        context: &["elm.json"],
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

    /// `target` and `project/target` in one rule: the nested pattern must
    /// still match when the plain one finds no context.
    #[test]
    fn every_matching_pattern_is_tried() {
        const NESTED: SimpleAdapter = SimpleAdapter {
            name: "nested",
            label: "Nested",
            rules: &[Rule {
                dirs: &["target", "project/target"],
                context: &["build.sbt"],
                unless_inside: &[],
            }],
        };
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.sbt"), "").unwrap();
        fs::create_dir_all(dir.path().join("project").join("target")).unwrap();
        let targets = scan_with(NESTED, dir.path());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].path, dir.path().join("project").join("target"));
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
