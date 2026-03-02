use clap::{Parser, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

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

    /// Skip size calculation to speed up scanning [default: off]
    #[arg(long, default_value_t = false, help_heading = "Performance")]
    pub no_size: bool,

    /// Generate shell completions for the given shell and exit
    #[arg(long = "generate-completions", value_name = "SHELL")]
    pub generate_completions: Option<Shell>,

    // ---- Safe adapters (default: on) ----------------------------------------
    /// Node.js/npm adapter — removes node_modules/ [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub node: bool,

    /// Rust/Cargo adapter — removes target/ [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub cargo: bool,

    /// Python adapter — removes __pycache__/, .venv/, dist/, etc. [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub python: bool,

    /// Go adapter — removes vendor/ [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub go: bool,

    /// Gradle adapter — removes .gradle/, build/ [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub gradle: bool,

    /// Maven adapter — removes target/ [default: on]
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub maven: bool,

    // ---- Dangerous adapters (default: off) -----------------------------------
    /// Gitignore adapter — removes all files matched by .gitignore (dangerous) [default: off]
    #[arg(
        long,
        default_value_t = false,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help_heading = "Adapters"
    )]
    pub gitignore: bool,
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum Mode {
    /// Show an interactive list and let the user choose what to delete
    Safe,
    /// Delete all found targets automatically without prompting
    Auto,
}
