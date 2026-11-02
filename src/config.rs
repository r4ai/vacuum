//! Defaults from `~/.config/vacuum/config.toml`.
//!
//! A flag given on the command line always wins over the config file.

use anyhow::Context as _;
use clap::ArgMatches;
use clap::parser::ValueSource;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::adapters::REGISTRY;
use crate::cli::{Cli, Mode};

/// The config file's contents. Every key is optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    mode: Option<String>,
    /// Added to `--exclude`.
    #[serde(default)]
    exclude: Vec<String>,
    no_size: Option<bool>,
    include_tracked: Option<bool>,
    trash: Option<bool>,
    one_file_system: Option<bool>,
    max_depth: Option<usize>,
    min_size: Option<String>,
    older_than: Option<String>,
    /// Adapter name → enabled.
    #[serde(default)]
    adapters: BTreeMap<String, bool>,
}

/// `$XDG_CONFIG_HOME/vacuum/config.toml`, or `~/.config/vacuum/config.toml`.
pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("vacuum").join("config.toml"))
}

impl Config {
    /// Read the config file; `None` when there is none.
    pub fn load() -> anyhow::Result<Option<Self>> {
        let Some(path) = path() else {
            return Ok(None);
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("Cannot read {}", path.display())),
        };
        let config = Self::parse(&text).with_context(|| format!("Invalid {}", path.display()))?;
        Ok(Some(config))
    }

    fn parse(text: &str) -> anyhow::Result<Self> {
        let config: Self = toml::from_str(text)?;
        for name in config.adapters.keys() {
            if !REGISTRY.iter().any(|spec| spec.name == name) {
                anyhow::bail!("unknown adapter {name:?} in [adapters]");
            }
        }
        Ok(config)
    }

    /// Fill every setting of `cli` that was not given on the command line.
    pub fn apply(self, cli: &mut Cli, matches: &ArgMatches) -> anyhow::Result<()> {
        let unset = |id: &str| matches.value_source(id) != Some(ValueSource::CommandLine);

        if let Some(mode) = self.mode
            && unset("mode")
        {
            cli.mode = <Mode as clap::ValueEnum>::from_str(&mode, true)
                .map_err(|_| anyhow::anyhow!("unknown mode {mode:?}"))?;
        }
        let mut exclude = self.exclude;
        exclude.append(&mut cli.exclude);
        cli.exclude = exclude;

        for (flag, value, field) in [
            ("no_size", self.no_size, &mut cli.no_size),
            (
                "include_tracked",
                self.include_tracked,
                &mut cli.include_tracked,
            ),
            ("trash", self.trash, &mut cli.trash),
            (
                "one_file_system",
                self.one_file_system,
                &mut cli.one_file_system,
            ),
        ] {
            if let Some(value) = value
                && unset(flag)
            {
                *field = value;
            }
        }
        if let Some(depth) = self.max_depth
            && unset("max_depth")
        {
            cli.max_depth = Some(depth);
        }
        if let Some(size) = self.min_size
            && unset("min_size")
        {
            cli.min_size = Some(
                size.parse()
                    .map_err(|e| anyhow::anyhow!("invalid min_size {size:?}: {e}"))?,
            );
        }
        if let Some(duration) = self.older_than
            && unset("older_than")
        {
            cli.older_than =
                Some(crate::age::parse_duration(&duration).map_err(|e| anyhow::anyhow!(e))?);
        }
        // `--min-size` needs sizes, so it cannot go with `--no-size`. clap
        // checked the command line alone; resolve what the config added.
        if cli.no_size && cli.min_size.is_some() {
            match (unset("no_size"), unset("min_size")) {
                (true, true) => anyhow::bail!("no_size and min_size cannot be set together"),
                (true, false) => cli.no_size = false,
                (false, _) => cli.min_size = None,
            }
        }
        for (name, enabled) in self.adapters {
            let spec = REGISTRY
                .iter()
                .find(|spec| spec.name == name)
                .expect("checked in parse");
            if unset(spec.name) {
                if enabled {
                    cli.adapters.enabled.insert(spec.name);
                } else {
                    cli.adapters.enabled.remove(spec.name);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory as _, FromArgMatches as _};

    fn cli_with(config: &str, args: &[&str]) -> Cli {
        let matches = Cli::command()
            .try_get_matches_from(std::iter::once("vacuum").chain(args.iter().copied()))
            .unwrap();
        let mut cli = Cli::from_arg_matches(&matches).unwrap();
        Config::parse(config)
            .unwrap()
            .apply(&mut cli, &matches)
            .unwrap();
        cli
    }

    #[test]
    fn config_fills_unset_options() {
        let cli = cli_with("max_depth = 3\nmin_size = \"1MB\"\n", &[]);
        assert_eq!(cli.max_depth, Some(3));
        assert_eq!(cli.min_size.map(|s| s.as_u64()), Some(1_000_000));
        assert!(cli_with("no_size = true\n", &[]).no_size);
    }

    #[test]
    fn command_line_wins_over_config() {
        let cli = cli_with(
            "max_depth = 3\nmode = \"list\"\n",
            &["--max-depth", "5", "-m", "auto"],
        );
        assert_eq!(cli.max_depth, Some(5));
        assert_eq!(cli.mode, Mode::Auto);
    }

    #[test]
    fn config_excludes_come_before_command_line_excludes() {
        let cli = cli_with("exclude = [\"a\"]\n", &["--exclude", "b"]);
        assert_eq!(cli.exclude, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn unknown_keys_and_adapters_are_rejected() {
        assert!(Config::parse("mdoe = \"list\"\n").is_err());
        assert!(Config::parse("[adapters]\nnodee = true\n").is_err());
    }
}
