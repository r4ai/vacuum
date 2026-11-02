use clap::{Arg, ArgMatches, Args, Command, FromArgMatches, Parser, ValueEnum, value_parser};
use clap_complete::Shell;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::adapters::REGISTRY;
use crate::cleaner::DeleteMethod;

#[derive(Parser, Debug)]
#[command(
    name = "vacuum",
    version,
    about = "Clean up unnecessary build artifacts and temp files",
    long_about = "vacuum scans a directory recursively and removes unnecessary build \
                  artifacts, dependency caches, and temporary files.\n\n\
                  By default all safe adapters are enabled and interactive (safe) mode \
                  is used so you can review candidates before deletion.\n\n\
                  Each adapter flag accepts an optional boolean value:\n  \
                  --node        enable (same as --node=true)\n  \
                  --node=false  disable"
)]
pub struct Cli {
    /// Directory to scan
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Cleanup mode
    #[arg(short = 'm', long, value_enum, default_value_t = Mode::Safe)]
    pub mode: Mode,

    /// Show what would be deleted without actually deleting anything
    #[arg(long)]
    pub dry_run: bool,

    /// Exclude paths matching GLOB (gitignore syntax, relative to PATH).
    /// Can be given more than once. `PATH/.vacuumignore` is also read.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Also list targets that contain files tracked by git [default: off]
    #[arg(long)]
    pub include_tracked: bool,

    /// Move targets to the OS trash instead of deleting them permanently
    #[arg(long)]
    pub trash: bool,

    /// Skip size calculation to speed up scanning [default: off]
    #[arg(long, default_value_t = false, help_heading = "Performance")]
    pub no_size: bool,

    /// Generate shell completions for the given shell and exit
    #[arg(long = "generate-completions", value_name = "SHELL")]
    pub generate_completions: Option<Shell>,

    #[command(flatten)]
    pub adapters: AdapterFlags,
}

impl Cli {
    pub fn delete_method(&self) -> DeleteMethod {
        if self.trash {
            DeleteMethod::Trash
        } else {
            DeleteMethod::Remove
        }
    }
}

/// The adapter flags (`--node`, `--gitignore=false`, ...), generated from
/// [`REGISTRY`].
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterFlags {
    /// Names of the enabled adapters.
    pub enabled: HashSet<&'static str>,
}

impl Args for AdapterFlags {
    fn augment_args(cmd: Command) -> Command {
        REGISTRY.iter().fold(cmd, |cmd, spec| {
            let default = if spec.default_enabled { "on" } else { "off" };
            let danger = if spec.is_safe { "" } else { " (dangerous)" };
            cmd.arg(
                Arg::new(spec.name)
                    .long(spec.name)
                    .value_parser(value_parser!(bool))
                    .num_args(0..=1)
                    .default_value(if spec.default_enabled {
                        "true"
                    } else {
                        "false"
                    })
                    .default_missing_value("true")
                    .value_name("BOOL")
                    .help_heading("Adapters")
                    .help(format!(
                        "{} adapter — removes {}{danger} [default: {default}]",
                        spec.label, spec.targets
                    )),
            )
        })
    }

    fn augment_args_for_update(cmd: Command) -> Command {
        Self::augment_args(cmd)
    }
}

impl FromArgMatches for AdapterFlags {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let enabled = REGISTRY
            .iter()
            .filter(|spec| {
                matches
                    .get_one::<bool>(spec.name)
                    .copied()
                    .unwrap_or(spec.default_enabled)
            })
            .map(|spec| spec.name)
            .collect();
        Ok(Self { enabled })
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum Mode {
    /// Show an interactive list and let the user choose what to delete
    Safe,
    /// Delete all found targets automatically without prompting
    Auto,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled(args: &[&str]) -> Vec<&'static str> {
        let cli =
            Cli::try_parse_from(std::iter::once("vacuum").chain(args.iter().copied())).unwrap();
        let mut names: Vec<_> = cli.adapters.enabled.into_iter().collect();
        names.sort();
        names
    }

    #[test]
    fn safe_adapters_are_enabled_by_default() {
        assert_eq!(
            enabled(&[]),
            vec!["cargo", "go", "gradle", "maven", "node", "ocaml", "python"]
        );
    }

    #[test]
    fn flag_without_value_enables_adapter() {
        assert!(enabled(&["--gitignore"]).contains(&"gitignore"));
    }

    #[test]
    fn flag_with_false_disables_adapter() {
        assert!(!enabled(&["--node=false"]).contains(&"node"));
    }

    #[test]
    fn invalid_flag_value_is_rejected() {
        assert!(Cli::try_parse_from(["vacuum", "--node=maybe"]).is_err());
    }
}
