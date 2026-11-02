use clap::{CommandFactory as _, Parser as _};
use std::process::ExitCode;

mod adapter;
mod adapters;
mod age;
mod cleaner;
mod cli;
mod display;
mod exclude;
mod interactive;
mod report;
mod roots;
mod scanner;
mod tracked;

use cli::{Cli, Mode};
use interactive::TuiResult;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Print the error chain without a backtrace, even when
            // `RUST_BACKTRACE` is set: these are user-facing errors.
            eprintln!("Error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Shell completion generation — exits immediately
    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "vacuum", &mut std::io::stdout());
        return Ok(());
    }

    if cli.json && cli.mode == Mode::Safe {
        anyhow::bail!("--json needs --mode list or --mode auto");
    }

    let roots = roots::Roots::resolve(&cli.paths)?;

    // Reject invalid exclude patterns before starting either mode.
    for root in &roots.dirs {
        exclude::Excludes::new(root, &cli.exclude)?;
    }

    match cli.mode {
        // ── Safe mode: full TUI workflow ──────────────────────────────────────
        Mode::Safe => match interactive::run_tui_flow(&roots, &cli)? {
            TuiResult::Cancelled => display::print_cancelled(),
            TuiResult::Completed { freed, errors } => {
                display::print_final_report(freed, &errors, cli.dry_run, cli.delete_method());
                if !errors.is_empty() {
                    anyhow::bail!("failed to delete {} target(s)", errors.len());
                }
            }
        },

        // ── JSON output: list mode or auto mode ──────────────────────────────
        Mode::List | Mode::Auto if cli.json => {
            let targets = scanner::scan_enabled(&roots.dirs, &cli)?;
            let dry_run = cli.dry_run || cli.mode == Mode::List;
            let outcome = if dry_run {
                cleaner::DeleteReport::default()
            } else {
                cleaner::remove_all(&targets, cli.delete_method(), |_| {})
            };
            println!(
                "{}",
                report::to_json(&targets, &outcome, dry_run, cli.delete_method())
            );
            if !outcome.errors.is_empty() {
                anyhow::bail!("failed to delete {} target(s)", outcome.errors.len());
            }
        }

        // ── List mode: print the targets, never delete ───────────────────────
        Mode::List => {
            let targets = scanner::scan_enabled(&roots.dirs, &cli)?;
            if cli.quiet {
                display::print_summary(&targets, 0, 0, true, cli.delete_method());
            } else {
                display::print_targets(&targets, &roots.base);
            }
        }

        // ── Auto mode: non-interactive scan + delete ─────────────────────────
        Mode::Auto => {
            if !scanner::ScanConfig::from(&cli).any_enabled() {
                println!("All adapters are disabled. Nothing to do.");
                return Ok(());
            }

            if !cli.quiet {
                let names: Vec<String> =
                    roots.dirs.iter().map(|d| d.display().to_string()).collect();
                eprintln!("Scanning {} ...", names.join(", "));
            }
            let targets = scanner::scan_enabled(&roots.dirs, &cli)?;

            if !cli.quiet {
                display::print_targets(&targets, &roots.base);
            }

            let method = cli.delete_method();
            let report = cleaner::delete_targets(&targets, cli.dry_run, method, cli.quiet);
            let removed = targets.len() - report.errors.len();
            display::print_summary(&targets, report.freed, removed, cli.dry_run, method);
            if !report.errors.is_empty() {
                anyhow::bail!("failed to delete {} target(s)", report.errors.len());
            }
        }
    }

    Ok(())
}
