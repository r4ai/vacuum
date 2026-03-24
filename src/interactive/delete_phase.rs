use std::sync::mpsc;
use std::time::Instant;

use bytesize::ByteSize;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::adapter::CleanTarget;
use crate::cleaner::{DeleteMsg, delete_with_progress};

// ─── State ────────────────────────────────────────────────────────────────────

pub struct DeletingState {
    pub rx: mpsc::Receiver<DeleteMsg>,
    pub total: usize,
    pub done: usize,
    pub current_path: String,
    pub freed: u64,
    pub errors: Vec<(String, String)>,
    pub finished: bool,
    pub start: Instant,
    pub dry_run: bool,
}

impl DeletingState {
    /// Spawn background delete thread and return the state.
    pub fn new(targets: Vec<CleanTarget>, dry_run: bool) -> Self {
        let total = targets.len();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            delete_with_progress(targets, dry_run, tx);
        });
        Self {
            rx,
            total,
            done: 0,
            current_path: String::new(),
            freed: 0,
            errors: Vec::new(),
            finished: false,
            start: Instant::now(),
            dry_run,
        }
    }

    /// Drain all pending messages from the background thread.
    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                DeleteMsg::Progress { path, freed, done } => {
                    self.current_path = path;
                    self.freed = freed;
                    self.done = done;
                }
                DeleteMsg::Done { freed, errors } => {
                    self.freed = freed;
                    self.errors = errors;
                    self.finished = true;
                }
            }
        }
    }
}

// ─── Rendering ───────────────────────────────────────────────────────────────

pub fn render_deleting(frame: &mut ratatui::Frame, state: &DeletingState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // body
            Constraint::Length(1), // hint
        ])
        .split(area);

    render_delete_header(frame, chunks[0], state.dry_run);
    render_delete_body(frame, state, chunks[1]);
    render_delete_hint(frame, chunks[2], state.finished);
}

fn render_delete_header(frame: &mut ratatui::Frame, area: Rect, dry_run: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let label = if dry_run { "Dry run..." } else { "Deleting..." };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" — {label}")),
    ]))
    .block(block);
    frame.render_widget(header, area);
}

fn render_delete_body(frame: &mut ratatui::Frame, state: &DeletingState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // spacing
            Constraint::Length(1), // current file
            Constraint::Length(1), // freed / errors
        ])
        .split(inner);

    // Progress bar
    let ratio = if state.total == 0 {
        1.0
    } else {
        state.done as f64 / state.total as f64
    };
    let label = format!("{} / {}", state.done, state.total);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    frame.render_widget(gauge, chunks[0]);

    // Current file
    let path_display = if state.current_path.is_empty() {
        Span::styled("Waiting...", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&state.current_path, Style::default().fg(Color::White))
    };
    frame.render_widget(Paragraph::new(Line::from(path_display)), chunks[2]);

    // Stats
    let elapsed = state.start.elapsed().as_secs_f64();
    let freed_str = ByteSize(state.freed).to_string();
    let stats_line = if state.finished {
        let status = if state.dry_run {
            "Dry run complete"
        } else {
            "Done"
        };
        Line::from(vec![
            Span::styled(
                format!("✓ {status}"),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  freed: {freed_str}")),
            if state.errors.is_empty() {
                Span::raw(String::new())
            } else {
                Span::styled(
                    format!("  {} error(s)", state.errors.len()),
                    Style::default().fg(Color::Red),
                )
            },
        ])
    } else {
        Line::from(vec![
            Span::raw(format!("  {:.1}s  freed: {freed_str}", elapsed)),
            if !state.errors.is_empty() {
                Span::styled(
                    format!("  {} error(s)", state.errors.len()),
                    Style::default().fg(Color::Red),
                )
            } else {
                Span::raw(String::new())
            },
        ])
    };
    frame.render_widget(Paragraph::new(stats_line), chunks[3]);
}

fn render_delete_hint(frame: &mut ratatui::Frame, area: Rect, finished: bool) {
    let hint = if finished {
        Line::from(Span::styled(
            " Returning to terminal...",
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(Span::styled(
            " Deleting — please wait...",
            Style::default().fg(Color::DarkGray),
        ))
    };
    frame.render_widget(ratatui::widgets::Paragraph::new(hint), area);
}
