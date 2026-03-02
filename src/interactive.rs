use std::io;
use std::path::Path;

use anyhow::Context as _;
use bytesize::ByteSize;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, ExecutableCommand as _};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Terminal;

use crate::adapter::CleanTarget;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Adapter,
    Path,
    Size,
    Description,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    fn toggle(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            SortDir::Asc => " ↑",
            SortDir::Desc => " ↓",
        }
    }
}

struct App<'a> {
    targets: &'a [CleanTarget],
    root: &'a Path,
    selected: Vec<bool>, // indexed by original target index
    order: Vec<usize>,   // display row -> original target index
    table_state: TableState,
    sort_col: SortColumn,
    sort_dir: SortDir,
    page_size: usize, // updated each frame from rendered area
}

impl<'a> App<'a> {
    fn new(targets: &'a [CleanTarget], root: &'a Path) -> Self {
        let order: Vec<usize> = (0..targets.len()).collect();
        let mut table_state = TableState::default();
        if !targets.is_empty() {
            table_state.select(Some(0));
        }
        let mut app = Self {
            targets,
            root,
            selected: vec![true; targets.len()],
            order,
            table_state,
            sort_col: SortColumn::Path,
            sort_dir: SortDir::Asc,
            page_size: 10,
        };
        app.apply_sort();
        app
    }

    fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    fn clamp_cursor(&mut self) {
        let len = self.order.len();
        if len == 0 {
            self.table_state.select(None);
        } else {
            let cur = self.cursor().min(len - 1);
            self.table_state.select(Some(cur));
        }
    }

    fn move_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let cur = self.cursor();
        let next = if cur == 0 { len - 1 } else { cur - 1 };
        self.table_state.select(Some(next));
    }

    fn move_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let cur = self.cursor();
        let next = if cur + 1 >= len { 0 } else { cur + 1 };
        self.table_state.select(Some(next));
    }

    fn move_top(&mut self) {
        if !self.order.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    fn move_bottom(&mut self) {
        let len = self.order.len();
        if len > 0 {
            self.table_state.select(Some(len - 1));
        }
    }

    fn move_page_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = self.cursor().saturating_sub(self.page_size);
        self.table_state.select(Some(next));
    }

    fn move_page_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor() + self.page_size).min(len - 1);
        self.table_state.select(Some(next));
    }

    fn move_half_page_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = self.cursor().saturating_sub(self.page_size / 2);
        self.table_state.select(Some(next));
    }

    fn move_half_page_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor() + self.page_size / 2).min(len - 1);
        self.table_state.select(Some(next));
    }

    fn toggle(&mut self) {
        let cur = self.cursor();
        if let Some(&orig_idx) = self.order.get(cur) {
            self.selected[orig_idx] = !self.selected[orig_idx];
        }
    }

    fn select_all(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = true);
    }

    fn select_none(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = false);
    }

    fn sort_by(&mut self, col: SortColumn) {
        if self.sort_col == col {
            self.sort_dir = self.sort_dir.toggle();
        } else {
            self.sort_col = col;
            self.sort_dir = SortDir::Asc;
        }
        self.apply_sort();
    }

    fn apply_sort(&mut self) {
        let targets = self.targets;
        let root = self.root;
        let col = self.sort_col;
        let dir = self.sort_dir;

        self.order.sort_by(|&a, &b| {
            let ta = &targets[a];
            let tb = &targets[b];
            let cmp = match col {
                SortColumn::Adapter => ta.adapter.cmp(tb.adapter),
                SortColumn::Path => {
                    let pa = ta.path.strip_prefix(root).unwrap_or(&ta.path);
                    let pb = tb.path.strip_prefix(root).unwrap_or(&tb.path);
                    pa.cmp(pb)
                }
                SortColumn::Size => ta.size.cmp(&tb.size),
                SortColumn::Description => ta.description.cmp(&tb.description),
            };
            if dir == SortDir::Desc { cmp.reverse() } else { cmp }
        });
        self.clamp_cursor();
    }

    fn total_size(&self) -> u64 {
        self.targets.iter().map(|t| t.size).sum()
    }

    fn selected_size(&self) -> u64 {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.size)
            .sum()
    }

    fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&v| v).count()
    }

    fn chosen_targets(&self) -> Vec<CleanTarget> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.clone())
            .collect()
    }
}

fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // table
            Constraint::Length(2), // stats
            Constraint::Length(2), // keybind hint (2 lines)
        ])
        .split(area);

    // Update page_size from the visible table rows (subtract borders + header + margin)
    app.page_size = (chunks[1].height as usize).saturating_sub(4).max(1);

    // --- Header ---
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let header_text = Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — Select items to delete"),
    ]))
    .block(header_block);
    frame.render_widget(header_text, chunks[0]);

    // --- Table ---
    let col_defs: &[(&str, SortColumn)] = &[
        ("Adapter", SortColumn::Adapter),
        ("Path", SortColumn::Path),
        ("Size", SortColumn::Size),
        ("Description", SortColumn::Description),
    ];

    let header_cells = std::iter::once(
        Cell::from("").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    )
    .chain(col_defs.iter().map(|(name, col)| {
        let is_active = app.sort_col == *col;
        let label = if is_active {
            format!("{}{}", name, app.sort_dir.indicator())
        } else {
            name.to_string()
        };
        let style = if is_active {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        };
        Cell::from(label).style(style)
    }));
    let header_row = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .order
        .iter()
        .enumerate()
        .map(|(display_i, &orig_i)| {
            let t = &app.targets[orig_i];
            let sel = app.selected[orig_i];
            let is_cursor = app.table_state.selected() == Some(display_i);
            let check = if sel { "✓" } else { "✗" };
            let check_style = if sel {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let rel = t.path.strip_prefix(app.root).unwrap_or(&t.path);
            let row_style = if is_cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(check).style(check_style),
                Cell::from(t.adapter),
                Cell::from(rel.display().to_string()),
                Cell::from(ByteSize(t.size).to_string())
                    .style(Style::default().fg(Color::Cyan)),
                Cell::from(t.description.as_str()),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(12),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(header_row)
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(Style::default());
    frame.render_stateful_widget(table, chunks[1], &mut app.table_state);

    // --- Stats ---
    let sel_count = app.selected_count();
    let total_count = app.targets.len();
    let sel_size = ByteSize(app.selected_size());
    let total_size = ByteSize(app.total_size());
    let stats = Paragraph::new(Line::from(vec![
        Span::raw(" Selected: "),
        Span::styled(
            format!("{sel_size}"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" / {total_size}   ")),
        Span::styled(
            format!("{sel_count}"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" / {total_count} items")),
    ]));
    frame.render_widget(stats, chunks[2]);

    // --- Keybind hint (2 lines) ---
    let hint = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" [↑↓/jk]", Style::default().fg(Color::Yellow)),
            Span::raw(" Move  "),
            Span::styled("[PgUp/PgDn]", Style::default().fg(Color::Yellow)),
            Span::raw(" Page  "),
            Span::styled("[^u/^d]", Style::default().fg(Color::Yellow)),
            Span::raw(" Half  "),
            Span::styled("[g/G]", Style::default().fg(Color::Yellow)),
            Span::raw(" Top/Bot  "),
            Span::styled("[1-4]", Style::default().fg(Color::Yellow)),
            Span::raw(" Sort"),
        ]),
        Line::from(vec![
            Span::styled(" [Space]", Style::default().fg(Color::Yellow)),
            Span::raw(" Toggle  "),
            Span::styled("[a]", Style::default().fg(Color::Yellow)),
            Span::raw(" All  "),
            Span::styled("[n]", Style::default().fg(Color::Yellow)),
            Span::raw(" None  "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Confirm  "),
            Span::styled("[q/Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Quit"),
        ]),
    ]);
    frame.render_widget(hint, chunks[3]);
}

/// Present an interactive multi-select table and return the targets
/// the user chose for deletion.
pub fn select_targets(targets: &[CleanTarget], root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
    if targets.is_empty() {
        return Ok(vec![]);
    }

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    let mut app = App::new(targets, root);
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode().context("Failed to disable raw mode")?;
    terminal
        .backend_mut()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<Vec<CleanTarget>> {
    loop {
        terminal.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(vec![]),
                KeyCode::Enter => return Ok(app.chosen_targets()),

                // Movement
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::PageUp => app.move_page_up(),
                KeyCode::PageDown => app.move_page_down(),
                KeyCode::Home | KeyCode::Char('g') => app.move_top(),
                KeyCode::End | KeyCode::Char('G') => app.move_bottom(),
                KeyCode::Char('u')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    app.move_half_page_up()
                }
                KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    app.move_half_page_down()
                }

                // Selection
                KeyCode::Char(' ') => app.toggle(),
                KeyCode::Char('a') => app.select_all(),
                KeyCode::Char('n') => app.select_none(),

                // Sort by column (1=Adapter, 2=Path, 3=Size, 4=Description)
                KeyCode::Char('1') => app.sort_by(SortColumn::Adapter),
                KeyCode::Char('2') => app.sort_by(SortColumn::Path),
                KeyCode::Char('3') => app.sort_by(SortColumn::Size),
                KeyCode::Char('4') => app.sort_by(SortColumn::Description),

                _ => {}
            }
        }
    }
}
