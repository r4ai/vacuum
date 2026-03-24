use std::io;
use std::path::Path;

use anyhow::Context as _;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::cli::Cli;

mod adapter_select;
mod delete_phase;
mod scan_phase;
mod target_select;
pub mod types;

use adapter_select::{
    AdapterSelectionResult, AdapterSelectionState, handle_adapter_selection_key,
    render_adapter_selection,
};
use delete_phase::{DeletingState, render_deleting};
use scan_phase::{ScanningState, render_scanning};
use target_select::{
    App, handle_detail_key, handle_help_key, handle_normal_key, handle_search_key,
    handle_visual_key, render,
};
use types::{ActionResult, Mode};

// ─── Public result type ───────────────────────────────────────────────────────

pub enum TuiResult {
    Completed {
        freed: u64,
        errors: Vec<(String, String)>,
    },
    Cancelled,
}

// ─── Phase state machine ──────────────────────────────────────────────────────

enum Phase {
    AdapterSelection(AdapterSelectionState),
    Scanning(ScanningState),
    TargetSelection(App),
    Deleting(DeletingState),
}

// ─── Main event loop ──────────────────────────────────────────────────────────

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    root: &Path,
    cli: &Cli,
) -> anyhow::Result<TuiResult> {
    let mut phase = Phase::AdapterSelection(AdapterSelectionState::new(cli.no_size));

    loop {
        // Render current phase
        terminal.draw(|frame| match &mut phase {
            Phase::AdapterSelection(s) => render_adapter_selection(frame, s),
            Phase::Scanning(s) => render_scanning(frame, s),
            Phase::TargetSelection(app) => render(frame, app),
            Phase::Deleting(s) => render_deleting(frame, s),
        })?;

        // Drain background thread messages (non-blocking)
        match &mut phase {
            Phase::Scanning(s) => {
                s.drain();
                // Auto-transition when scan completes
                if s.done {
                    let targets = std::mem::take(&mut s.found);
                    if targets.is_empty() {
                        return Ok(TuiResult::Completed {
                            freed: 0,
                            errors: vec![],
                        });
                    }
                    phase = Phase::TargetSelection(App::new(targets, root.to_path_buf()));
                    continue;
                }
            }
            Phase::Deleting(s) => {
                let was_finished = s.finished;
                s.drain();
                // Auto-exit after Done state has been rendered at least once
                if was_finished && s.finished {
                    let freed = s.freed;
                    let errors = std::mem::take(&mut s.errors);
                    return Ok(TuiResult::Completed { freed, errors });
                }
            }
            _ => {}
        }

        // Poll events with a short timeout so spinners animate
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Mouse(mouse) => {
                if let Phase::TargetSelection(app) = &mut phase
                    && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                {
                    target_select::handle_mouse(app, mouse.column, mouse.row, mouse.kind);
                }
            }

            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match &mut phase {
                    // ── Adapter selection ─────────────────────────────────
                    Phase::AdapterSelection(state) => {
                        match handle_adapter_selection_key(state, key) {
                            AdapterSelectionResult::Quit => return Ok(TuiResult::Cancelled),
                            AdapterSelectionResult::Confirm => {
                                let cfg = state.cfg.clone();
                                phase =
                                    Phase::Scanning(ScanningState::new(root.to_path_buf(), cfg));
                            }
                            AdapterSelectionResult::Continue => {}
                        }
                    }

                    // ── Scanning ──────────────────────────────────────────
                    Phase::Scanning(_) => {
                        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                            return Ok(TuiResult::Cancelled);
                        }
                    }

                    // ── Target selection ──────────────────────────────────
                    Phase::TargetSelection(app) => {
                        let result = match app.mode {
                            Mode::Normal => handle_normal_key(app, key),
                            Mode::Visual => handle_visual_key(app, key),
                            Mode::Search => {
                                handle_search_key(app, key);
                                ActionResult::Continue
                            }
                            Mode::Help => handle_help_key(app, key),
                            Mode::Detail => handle_detail_key(app, key),
                        };
                        match result {
                            ActionResult::Quit => return Ok(TuiResult::Cancelled),
                            ActionResult::Confirm => {
                                let chosen = app.chosen_targets();
                                if chosen.is_empty() {
                                    return Ok(TuiResult::Completed {
                                        freed: 0,
                                        errors: vec![],
                                    });
                                }
                                phase = Phase::Deleting(DeletingState::new(chosen, cli.dry_run));
                            }
                            ActionResult::Continue => {}
                        }
                    }

                    // ── Deleting ──────────────────────────────────────────
                    Phase::Deleting(state) => {
                        if state.finished
                            && matches!(
                                key.code,
                                KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc
                            )
                        {
                            let freed = state.freed;
                            let errors = std::mem::take(&mut state.errors);
                            return Ok(TuiResult::Completed { freed, errors });
                        }
                        // Ignore all keys while deletion is in progress
                    }
                }
            }

            _ => {}
        }
    }
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the full safe-mode TUI workflow:
/// adapter selection → scanning → target selection → deletion.
pub fn run_tui_flow(root: &Path, cli: &Cli) -> anyhow::Result<TuiResult> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    let result = run_loop(&mut terminal, root, cli);

    // Always restore terminal, even on error
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();

    result
}
