use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::scanner::ScanConfig;

// ─── Adapter metadata ────────────────────────────────────────────────────────

struct AdapterEntry {
    name: &'static str,
    description: &'static str,
    is_safe: bool,
    get: fn(&ScanConfig) -> bool,
    set: fn(&mut ScanConfig, bool),
}

static ALL_ADAPTERS: &[AdapterEntry] = &[
    AdapterEntry {
        name: "node",
        description: "node_modules/",
        is_safe: true,
        get: |c| c.node,
        set: |c, v| c.node = v,
    },
    AdapterEntry {
        name: "cargo",
        description: "target/",
        is_safe: true,
        get: |c| c.cargo,
        set: |c, v| c.cargo = v,
    },
    AdapterEntry {
        name: "python",
        description: "__pycache__/, .venv/",
        is_safe: true,
        get: |c| c.python,
        set: |c, v| c.python = v,
    },
    AdapterEntry {
        name: "go",
        description: "pkg/mod/ cache",
        is_safe: true,
        get: |c| c.go,
        set: |c, v| c.go = v,
    },
    AdapterEntry {
        name: "gradle",
        description: ".gradle/",
        is_safe: true,
        get: |c| c.gradle,
        set: |c, v| c.gradle = v,
    },
    AdapterEntry {
        name: "maven",
        description: ".m2/",
        is_safe: true,
        get: |c| c.maven,
        set: |c, v| c.maven = v,
    },
    AdapterEntry {
        name: "gitignore",
        description: ".gitignore-matched paths",
        is_safe: false,
        get: |c| c.gitignore,
        set: |c, v| c.gitignore = v,
    },
];

// ─── State ────────────────────────────────────────────────────────────────────

pub struct AdapterSelectionState {
    pub cfg: ScanConfig,
    cursor: usize,
    list_state: ListState,
}

impl AdapterSelectionState {
    /// Build state with safe adapters pre-selected.
    pub fn new(no_size: bool) -> Self {
        let mut cfg = ScanConfig {
            no_size,
            node: false,
            cargo: false,
            python: false,
            go: false,
            gradle: false,
            maven: false,
            gitignore: false,
        };
        for entry in ALL_ADAPTERS {
            if entry.is_safe {
                (entry.set)(&mut cfg, true);
            }
        }
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { cfg, cursor: 0, list_state }
    }

    pub fn any_selected(&self) -> bool {
        ALL_ADAPTERS.iter().any(|e| (e.get)(&self.cfg))
    }

    fn move_up(&mut self) {
        let len = ALL_ADAPTERS.len();
        self.cursor = if self.cursor == 0 { len - 1 } else { self.cursor - 1 };
        self.list_state.select(Some(self.cursor));
    }

    fn move_down(&mut self) {
        let len = ALL_ADAPTERS.len();
        self.cursor = if self.cursor + 1 >= len { 0 } else { self.cursor + 1 };
        self.list_state.select(Some(self.cursor));
    }

    fn toggle(&mut self) {
        if let Some(entry) = ALL_ADAPTERS.get(self.cursor) {
            let cur = (entry.get)(&self.cfg);
            (entry.set)(&mut self.cfg, !cur);
        }
    }
}

// ─── Rendering ───────────────────────────────────────────────────────────────

pub fn render_adapter_selection(frame: &mut ratatui::Frame, state: &mut AdapterSelectionState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // list
            Constraint::Length(1), // hint
        ])
        .split(area);

    // Header
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — Select adapters to scan"),
    ]))
    .block(block);
    frame.render_widget(header, chunks[0]);

    // Adapter list
    let items: Vec<ListItem> = ALL_ADAPTERS
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_cursor = i == state.cursor;
            let enabled = (entry.get)(&state.cfg);
            let check = if enabled { "✓" } else { "✗" };
            let check_style = if enabled {
                Style::default().fg(Color::Green)
            } else if is_cursor {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let safe_badge = if entry.is_safe {
                Span::styled(" [safe]", Style::default().fg(Color::Green))
            } else {
                Span::styled(" [custom]", Style::default().fg(Color::Yellow))
            };

            ListItem::new(Line::from(vec![
                Span::styled(check, check_style),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", entry.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    entry.description,
                    Style::default().fg(Color::Gray),
                ),
                safe_badge,
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);

    // Hint
    let k = |s| Span::styled(s, Style::default().fg(Color::Yellow));
    let t = |s: &'static str| Span::raw(s);
    let hint = Line::from(vec![
        k(" [↑↓/jk]"),
        t(" Move  "),
        k("[Space]"),
        t(" Toggle  "),
        Span::styled(
            "[Enter]",
            Style::default().fg(if state.any_selected() { Color::Green } else { Color::DarkGray }),
        ),
        t(" Scan  "),
        Span::styled("[q/Esc]", Style::default().fg(Color::Red)),
        t(" Quit"),
    ]);
    frame.render_widget(Paragraph::new(hint), chunks[2]);
}

// ─── Event handling ───────────────────────────────────────────────────────────

pub enum AdapterSelectionResult {
    Continue,
    Confirm,
    Quit,
}

pub fn handle_adapter_selection_key(
    state: &mut AdapterSelectionState,
    key: KeyEvent,
) -> AdapterSelectionResult {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => AdapterSelectionResult::Quit,
        KeyCode::Enter => {
            if state.any_selected() {
                AdapterSelectionResult::Confirm
            } else {
                AdapterSelectionResult::Continue
            }
        }
        KeyCode::Up | KeyCode::Char('k') => { state.move_up(); AdapterSelectionResult::Continue }
        KeyCode::Down | KeyCode::Char('j') => { state.move_down(); AdapterSelectionResult::Continue }
        KeyCode::Char(' ') => { state.toggle(); AdapterSelectionResult::Continue }
        _ => AdapterSelectionResult::Continue,
    }
}
