//! Every adapter is defined once, in [`REGISTRY`]. CLI flags, the scan
//! configuration and the TUI adapter list are all generated from it.

pub mod cargo;
pub mod gitignore;
pub mod go;
pub mod gradle;
pub mod maven;
pub mod node;
pub mod ocaml;
pub mod python;
pub mod simple;

pub use cargo::CargoAdapter;
pub use gitignore::GitignoreAdapter;
pub use go::GoAdapter;
pub use gradle::GradleAdapter;
pub use maven::MavenAdapter;
pub use node::NodeAdapter;
pub use ocaml::OcamlAdapter;
pub use python::PythonAdapter;

use crate::adapter::Adapter;

/// How an adapter finds its targets.
pub enum Kind {
    /// Judges single entries during the scanner's walk.
    Entry(&'static dyn Adapter),
    /// Runs its own walk; see [`GitignoreAdapter`].
    Gitignore,
}

/// The definition of one adapter.
pub struct AdapterSpec {
    /// Identifier used as the CLI flag (`--<name>`) and in the output.
    pub name: &'static str,
    /// Ecosystem shown in the CLI help (e.g. `"Node.js/npm"`).
    pub label: &'static str,
    /// What the adapter deletes, shown in the CLI help and the TUI.
    pub targets: &'static str,
    /// Whether the adapter is enabled when its flag is not given.
    pub default_enabled: bool,
    /// Whether the adapter only deletes regenerable artifacts.
    pub is_safe: bool,
    pub kind: Kind,
}

/// All adapters, in priority order: when two adapters match the same
/// entry, the earlier one wins.
pub static REGISTRY: &[AdapterSpec] = &[
    AdapterSpec {
        name: "node",
        label: "Node.js/npm",
        targets: "node_modules/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&NodeAdapter),
    },
    AdapterSpec {
        name: "js-cache",
        label: "JS framework",
        targets: ".next/, .nuxt/, .svelte-kit/, .turbo/, .parcel-cache/, .angular/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::JS_CACHE),
    },
    AdapterSpec {
        name: "cargo",
        label: "Rust/Cargo",
        targets: "target/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&CargoAdapter),
    },
    AdapterSpec {
        name: "python",
        label: "Python",
        targets: "__pycache__/, .venv/, venv/, dist/, build/, .eggs/, *.pyc",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&PythonAdapter),
    },
    AdapterSpec {
        name: "python-cache",
        label: "Python tools",
        targets: ".pytest_cache/, .mypy_cache/, .ruff_cache/, .tox/, .nox/, *.egg-info/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::PYTHON_CACHE),
    },
    AdapterSpec {
        name: "go",
        label: "Go",
        targets: "vendor/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&GoAdapter),
    },
    AdapterSpec {
        name: "gradle",
        label: "Gradle",
        targets: ".gradle/, build/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&GradleAdapter),
    },
    AdapterSpec {
        name: "maven",
        label: "Maven",
        targets: "target/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&MavenAdapter),
    },
    AdapterSpec {
        name: "ocaml",
        label: "OCaml/dune",
        targets: "_build/, _opam/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&OcamlAdapter),
    },
    AdapterSpec {
        name: "dotnet",
        label: ".NET",
        targets: "bin/, obj/ next to a *.csproj, *.fsproj or *.vbproj",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::DOTNET),
    },
    AdapterSpec {
        name: "swift",
        label: "Swift Package Manager",
        targets: ".build/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::SWIFT),
    },
    AdapterSpec {
        name: "haskell",
        label: "Haskell",
        targets: ".stack-work/, dist-newstyle/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::HASKELL),
    },
    AdapterSpec {
        name: "elixir",
        label: "Elixir",
        targets: "_build/, deps/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::ELIXIR),
    },
    AdapterSpec {
        name: "zig",
        label: "Zig",
        targets: ".zig-cache/, zig-cache/, zig-out/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::ZIG),
    },
    AdapterSpec {
        name: "dart",
        label: "Dart/Flutter",
        targets: ".dart_tool/, build/",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::DART),
    },
    AdapterSpec {
        name: "php",
        label: "PHP Composer",
        targets: "vendor/ next to a composer.json",
        default_enabled: true,
        is_safe: true,
        kind: Kind::Entry(&simple::PHP),
    },
    AdapterSpec {
        name: GitignoreAdapter::NAME,
        label: "Gitignore",
        targets: ".gitignore-matched paths",
        default_enabled: false,
        is_safe: false,
        kind: Kind::Gitignore,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = REGISTRY.iter().map(|spec| spec.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), REGISTRY.len());
    }

    #[test]
    fn entry_adapter_name_matches_registry() {
        for spec in REGISTRY {
            if let Kind::Entry(adapter) = spec.kind {
                assert_eq!(adapter.name(), spec.name);
            }
        }
    }

    #[test]
    fn readme_lists_every_adapter() {
        let readme = include_str!("../../README.md");
        for spec in REGISTRY {
            assert!(
                readme.contains(&format!("| `--{}`", spec.name)),
                "README adapter table lacks --{}",
                spec.name
            );
        }
    }
}
