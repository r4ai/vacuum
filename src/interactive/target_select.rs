use std::path::PathBuf;

use bytesize::ByteSize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
};

use crate::adapter::CleanTarget;

use super::types::{Action, ActionResult, KEYBINDINGS, Mode, SortColumn, SortDir};

// ─── App state ───────────────────────────────────────────────────────────────

pub struct App {
    pub targets: Vec<CleanTarget>,
    pub root: PathBuf,
    pub selected: Vec<bool>,
    pub sorted_order: Vec<usize>,
    pub order: Vec<usize>,
    pub table_state: TableState,
    pub visual_anchor: usize,
    pub sort_col: SortColumn,
    pub sort_dir: SortDir,
    pub page_size: usize,
    pub table_area: Rect,
    pub mode: Mode,
    pub filter_query: String,
    pub help_state: ListState,
    pub help_filter: String,
    pub help_order: Vec<usize>,
    pub help_searching: bool,
}

impl App {
    pub fn new(targets: Vec<CleanTarget>, root: PathBuf) -> Self {
        let sorted_order: Vec<usize> = (0..targets.len()).collect();
        let order = sorted_order.clone();
        let mut table_state = TableState::default();
        if !targets.is_empty() {
            table_state.select(Some(0));
        }
        let mut help_state = ListState::default();
        help_state.select(Some(0));
        let mut app = Self {
            targets,
            root,
            selected: vec![true; 0],
            sorted_order,
            order,
            table_state,
            sort_col: SortColumn::Path,
            sort_dir: SortDir::Asc,
            page_size: 10,
            visual_anchor: 0,
            table_area: Rect::default(),
            mode: Mode::Normal,
            filter_query: String::new(),
            help_state,
            help_filter: String::new(),
            help_order: (0..KEYBINDINGS.len()).collect(),
            help_searching: false,
        };
        app.selected = vec![true; app.targets.len()];
        app.apply_sort();
        app
    }

    pub fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn clamp_cursor(&mut self) {
        let len = self.order.len();
        if len == 0 {
            self.table_state.select(None);
            if self.mode == Mode::Visual {
                self.mode = Mode::Normal;
            }
        } else {
            let cur = self.cursor().min(len - 1);
            self.table_state.select(Some(cur));
            self.visual_anchor = self.visual_anchor.min(len - 1);
        }
    }

    pub fn enter_visual(&mut self) {
        if self.order.is_empty() {
            return;
        }
        self.visual_anchor = self.cursor();
        self.mode = Mode::Visual;
    }

    pub fn visual_range(&self) -> (usize, usize) {
        let cur = self.cursor();
        let anchor = self.visual_anchor;
        (
            anchor.min(cur),
            anchor.max(cur).min(self.order.len().saturating_sub(1)),
        )
    }

    pub fn apply_visual_toggle(&mut self) {
        let target = self
            .order
            .get(self.visual_anchor)
            .map(|&orig_i| !self.selected[orig_i])
            .unwrap_or(true);
        let (lo, hi) = self.visual_range();
        for display_i in lo..=hi {
            if let Some(&orig_i) = self.order.get(display_i) {
                self.selected[orig_i] = target;
            }
        }
        self.mode = Mode::Normal;
    }

    pub fn apply_visual_op(&mut self, target: bool) {
        let (lo, hi) = self.visual_range();
        for display_i in lo..=hi {
            if let Some(&orig_i) = self.order.get(display_i) {
                self.selected[orig_i] = target;
            }
        }
        self.mode = Mode::Normal;
    }

    pub fn move_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let cur = self.cursor();
        self.table_state
            .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
    }

    pub fn move_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let cur = self.cursor();
        self.table_state
            .select(Some(if cur + 1 >= len { 0 } else { cur + 1 }));
    }

    pub fn move_top(&mut self) {
        if !self.order.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    pub fn move_bottom(&mut self) {
        let len = self.order.len();
        if len > 0 {
            self.table_state.select(Some(len - 1));
        }
    }

    pub fn move_page_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = self.cursor().saturating_sub(self.page_size);
        self.table_state.select(Some(next));
    }

    pub fn move_page_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor() + self.page_size).min(len - 1);
        self.table_state.select(Some(next));
    }

    pub fn move_half_page_up(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = self.cursor().saturating_sub(self.page_size / 2);
        self.table_state.select(Some(next));
    }

    pub fn move_half_page_down(&mut self) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor() + self.page_size / 2).min(len - 1);
        self.table_state.select(Some(next));
    }

    pub fn toggle(&mut self) {
        let cur = self.cursor();
        if let Some(&orig_idx) = self.order.get(cur) {
            self.selected[orig_idx] = !self.selected[orig_idx];
        }
    }

    pub fn select_all(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = true);
    }

    pub fn select_none(&mut self) {
        self.selected.iter_mut().for_each(|v| *v = false);
    }

    pub fn sort_by(&mut self, col: SortColumn) {
        if self.sort_col == col {
            self.sort_dir = self.sort_dir.toggle();
        } else {
            self.sort_col = col;
            self.sort_dir = SortDir::Asc;
        }
        self.apply_sort();
    }

    pub fn apply_sort(&mut self) {
        if self.mode == Mode::Visual {
            self.mode = Mode::Normal;
        }
        let col = self.sort_col;
        let dir = self.sort_dir;
        let targets = self.targets.as_slice();
        let root = self.root.as_path();
        self.sorted_order.sort_by(|&a, &b| {
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
            if dir == SortDir::Desc {
                cmp.reverse()
            } else {
                cmp
            }
        });
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let query = self.filter_query.to_lowercase();
        if query.is_empty() {
            self.order = self.sorted_order.clone();
        } else {
            let targets = self.targets.as_slice();
            let root = self.root.as_path();
            self.order = self
                .sorted_order
                .iter()
                .copied()
                .filter(|&i| {
                    let t = &targets[i];
                    let rel = t.path.strip_prefix(root).unwrap_or(&t.path);
                    t.adapter.to_lowercase().contains(&query)
                        || rel.to_string_lossy().to_lowercase().contains(&query)
                        || t.description.to_lowercase().contains(&query)
                })
                .collect();
        }
        self.clamp_cursor();
    }

    pub fn apply_help_filter(&mut self) {
        let q = self.help_filter.to_lowercase();
        self.help_order = if q.is_empty() {
            (0..KEYBINDINGS.len()).collect()
        } else {
            KEYBINDINGS
                .iter()
                .enumerate()
                .filter(|(_, kb)| {
                    kb.key.to_lowercase().contains(&q) || kb.desc.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect()
        };
        let len = self.help_order.len();
        if len == 0 {
            self.help_state.select(None);
        } else {
            let cur = self.help_state.selected().unwrap_or(0).min(len - 1);
            self.help_state.select(Some(cur));
        }
    }

    pub fn help_move_up(&mut self) {
        let len = self.help_order.len();
        if len == 0 {
            return;
        }
        let cur = self.help_state.selected().unwrap_or(0);
        self.help_state
            .select(Some(if cur == 0 { len - 1 } else { cur - 1 }));
    }

    pub fn help_move_down(&mut self) {
        let len = self.help_order.len();
        if len == 0 {
            return;
        }
        let cur = self.help_state.selected().unwrap_or(0);
        self.help_state
            .select(Some(if cur + 1 >= len { 0 } else { cur + 1 }));
    }

    pub fn execute_action(&mut self, action: Action) -> ActionResult {
        match action {
            Action::MoveUp => self.move_up(),
            Action::MoveDown => self.move_down(),
            Action::MoveTop => self.move_top(),
            Action::MoveBottom => self.move_bottom(),
            Action::MovePageUp => self.move_page_up(),
            Action::MovePageDown => self.move_page_down(),
            Action::MoveHalfPageUp => self.move_half_page_up(),
            Action::MoveHalfPageDown => self.move_half_page_down(),
            Action::Toggle => self.toggle(),
            Action::SelectAll => self.select_all(),
            Action::SelectNone => self.select_none(),
            Action::EnterVisual => self.enter_visual(),
            Action::OpenDetail => {
                if !self.order.is_empty() {
                    self.mode = Mode::Detail;
                }
            }
            Action::SortByAdapter => self.sort_by(SortColumn::Adapter),
            Action::SortByPath => self.sort_by(SortColumn::Path),
            Action::SortBySize => self.sort_by(SortColumn::Size),
            Action::SortByDescription => self.sort_by(SortColumn::Description),
            Action::OpenSearch => self.mode = Mode::Search,
            Action::OpenHelp => {
                self.mode = if self.mode == Mode::Help {
                    Mode::Normal
                } else {
                    Mode::Help
                }
            }
            Action::Confirm => return ActionResult::Confirm,
            Action::Quit => return ActionResult::Quit,
        }
        ActionResult::Continue
    }

    pub fn total_size(&self) -> u64 {
        self.targets.iter().map(|t| t.size).sum()
    }

    pub fn selected_size(&self) -> u64 {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.size)
            .sum()
    }

    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&v| v).count()
    }

    pub fn chosen_targets(&self) -> Vec<CleanTarget> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, t)| t.clone())
            .collect()
    }
}

// ─── Rendering ───────────────────────────────────────────────────────────────

fn highlight_matches(text: &str, query: &str) -> Line<'static> {
    if query.is_empty() {
        return Line::from(text.to_string());
    }
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = text_lower[pos..].find(&query_lower) {
        let start = pos + rel;
        let end = start + query_lower.len();
        if start > pos {
            spans.push(Span::raw(text[pos..start].to_string()));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        pos = end;
    }
    if pos < text.len() {
        spans.push(Span::raw(text[pos..].to_string()));
    }
    Line::from(spans)
}

pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vert = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vert[1])[1]
}

pub fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(3),    // table
            Constraint::Length(1), // search bar
            Constraint::Length(1), // stats
            Constraint::Length(1), // hint
        ])
        .split(area);

    app.page_size = (chunks[1].height as usize).saturating_sub(4).max(1);

    render_header(frame, chunks[0]);
    render_table(frame, app, chunks[1]);
    render_search_bar(frame, app, chunks[2]);
    render_stats(frame, app, chunks[3]);
    render_hint(frame, app, chunks[4]);

    if app.mode == Mode::Help {
        render_help_overlay(frame, app, area);
    }
    if app.mode == Mode::Detail {
        render_detail_popup(frame, app, area);
    }
}

fn render_header(frame: &mut ratatui::Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let text = Paragraph::new(Line::from(vec![
        Span::styled(
            "vacuum",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — Select items to delete"),
    ]))
    .block(block);
    frame.render_widget(text, area);
}

fn sort_column_at(col: u16, table_area: Rect) -> Option<SortColumn> {
    let inner_x = col.saturating_sub(table_area.x + 1);
    let inner_w = table_area.width.saturating_sub(2);

    let rects = Layout::horizontal([
        Constraint::Length(3),
        Constraint::Length(12),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Fill(1),
    ])
    .split(Rect {
        x: 0,
        y: 0,
        width: inner_w,
        height: 1,
    });

    if inner_x < rects[1].x {
        return None;
    }
    if inner_x < rects[2].x {
        return Some(SortColumn::Adapter);
    }
    if inner_x < rects[3].x {
        return Some(SortColumn::Path);
    }
    if inner_x < rects[4].x {
        return Some(SortColumn::Size);
    }
    if inner_x < inner_w {
        return Some(SortColumn::Description);
    }
    None
}

fn render_table(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    app.table_area = area;
    let col_defs: &[(&str, SortColumn)] = &[
        ("Adapter", SortColumn::Adapter),
        ("Path", SortColumn::Path),
        ("Size", SortColumn::Size),
        ("Description", SortColumn::Description),
    ];

    let header_cells = std::iter::once(
        Cell::from("").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
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
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        };
        Cell::from(label).style(style)
    }));
    let header_row = Row::new(header_cells).height(1).bottom_margin(1);

    let targets = &app.targets;
    let root = &app.root;
    let rows: Vec<Row> = app
        .order
        .iter()
        .enumerate()
        .map(|(display_i, &orig_i)| {
            let t = &targets[orig_i];
            let sel = app.selected[orig_i];
            let is_cursor = app.table_state.selected() == Some(display_i);
            let is_in_visual = app.mode == Mode::Visual && {
                let (lo, hi) = app.visual_range();
                lo <= display_i && display_i <= hi
            };
            let check = if sel { "✓" } else { "✗" };
            let check_style = if sel {
                Style::default().fg(Color::Green)
            } else if is_cursor || is_in_visual {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let rel = t.path.strip_prefix(root.as_path()).unwrap_or(&t.path);
            let query = &app.filter_query;
            let row_style = if is_cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if is_in_visual {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(check).style(check_style),
                Cell::from(highlight_matches(t.adapter, query)),
                Cell::from(highlight_matches(&rel.display().to_string(), query)),
                Cell::from(ByteSize(t.size).to_string()).style(Style::default().fg(Color::Cyan)),
                Cell::from(highlight_matches(&t.description, query)),
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
    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_search_bar(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let (text, style) = match app.mode {
        Mode::Search => (
            format!(" / {}█", app.filter_query),
            Style::default().fg(Color::White),
        ),
        _ if !app.filter_query.is_empty() => (
            format!(
                " / {} (active filter — press / to edit, Esc to clear)",
                app.filter_query
            ),
            Style::default().fg(Color::Yellow),
        ),
        _ => (
            " / type to filter".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    };
    frame.render_widget(Paragraph::new(text).style(style), area);
}

fn render_stats(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let sel_count = app.selected_count();
    let total_count = app.targets.len();
    let visible_count = app.order.len();
    let sel_size = ByteSize(app.selected_size());
    let total_size = ByteSize(app.total_size());

    let mut spans = vec![
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
    ];
    if visible_count < total_count {
        spans.push(Span::styled(
            format!("  ({visible_count} shown)"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_hint(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let k = |s| Span::styled(s, Style::default().fg(Color::Yellow));
    let t = |s| Span::raw(s);

    let line = if app.mode == Mode::Visual {
        Line::from(vec![
            Span::styled(
                " VISUAL",
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD),
            ),
            t("  "),
            k("[↑↓/jk]"),
            t(" Extend  "),
            k("[Space]"),
            t(" Toggle  "),
            k("[a/n]"),
            t(" Sel/Desel  "),
            Span::styled("[v/Esc]", Style::default().fg(Color::Red)),
            t(" Exit"),
        ])
    } else {
        Line::from(vec![
            k(" [↑↓/jk]"),
            t(" Move  "),
            k("[Space]"),
            t(" Toggle  "),
            k("[v]"),
            t(" Visual  "),
            k("[a/n]"),
            t(" All/None  "),
            k("[/]"),
            t(" Filter  "),
            k("[?]"),
            t(" Help  "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            t(" Confirm  "),
            Span::styled("[q]", Style::default().fg(Color::Red)),
            t(" Quit"),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_help_overlay(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let popup = popup_area(area, 60, 80);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keybindings  [↑↓/jk] Navigate  [Enter] Execute  [/] Search  [?/q/Esc] Close ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let (search_text, search_style) = if app.help_searching {
        (
            format!(" / {}█", app.help_filter),
            Style::default().fg(Color::White),
        )
    } else if !app.help_filter.is_empty() {
        (
            format!(" / {} (press / to edit, Esc to clear)", app.help_filter),
            Style::default().fg(Color::Yellow),
        )
    } else {
        (
            " / type to filter keybindings".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };
    frame.render_widget(
        Paragraph::new(search_text).style(search_style),
        inner_chunks[1],
    );

    let items: Vec<ListItem> = app
        .help_order
        .iter()
        .map(|&kb_i| {
            let kb = &KEYBINDINGS[kb_i];
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<12}", kb.key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(kb.desc),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, inner_chunks[0], &mut app.help_state);
}

fn render_detail_popup(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let popup = popup_area(area, 70, 50);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Row Details  [any key] Close ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let cur = app.cursor();
    let Some(&orig_i) = app.order.get(cur) else {
        return;
    };
    let t = &app.targets[orig_i];
    let rel = t.path.strip_prefix(app.root.as_path()).unwrap_or(&t.path);

    let fields: &[(&str, String)] = &[
        ("Adapter", t.adapter.to_string()),
        ("Path", rel.display().to_string()),
        ("Size", ByteSize(t.size).to_string()),
        ("Description", t.description.clone()),
    ];

    let rows: Vec<Row> = fields
        .iter()
        .map(|(label, value)| {
            Row::new(vec![
                Cell::from(*label).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(value.clone()),
            ])
        })
        .collect();

    let table = Table::new(rows, [Constraint::Length(13), Constraint::Min(10)]);
    frame.render_widget(table, inner);
}

// ─── Event handling ───────────────────────────────────────────────────────────

pub fn handle_mouse(app: &mut App, col: u16, row: u16, kind: MouseEventKind) {
    if kind == MouseEventKind::Down(MouseButton::Left)
        && row == app.table_area.y + 1
        && app.mode != Mode::Search
        && !app.help_searching
        && let Some(sc) = sort_column_at(col, app.table_area)
    {
        app.sort_by(sc);
    }
}

pub fn handle_detail_key(app: &mut App, _key: KeyEvent) -> ActionResult {
    app.mode = Mode::Normal;
    ActionResult::Continue
}

pub fn handle_normal_key(app: &mut App, key: KeyEvent) -> ActionResult {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => ActionResult::Quit,
        KeyCode::Enter => ActionResult::Confirm,
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            ActionResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            ActionResult::Continue
        }
        KeyCode::PageUp => {
            app.move_page_up();
            ActionResult::Continue
        }
        KeyCode::PageDown => {
            app.move_page_down();
            ActionResult::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.move_top();
            ActionResult::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.move_bottom();
            ActionResult::Continue
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_half_page_up();
            ActionResult::Continue
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_half_page_down();
            ActionResult::Continue
        }
        KeyCode::Char(' ') => {
            app.toggle();
            ActionResult::Continue
        }
        KeyCode::Char('a') => {
            app.select_all();
            ActionResult::Continue
        }
        KeyCode::Char('n') => {
            app.select_none();
            ActionResult::Continue
        }
        KeyCode::Char('1') => {
            app.sort_by(SortColumn::Adapter);
            ActionResult::Continue
        }
        KeyCode::Char('2') => {
            app.sort_by(SortColumn::Path);
            ActionResult::Continue
        }
        KeyCode::Char('3') => {
            app.sort_by(SortColumn::Size);
            ActionResult::Continue
        }
        KeyCode::Char('4') => {
            app.sort_by(SortColumn::Description);
            ActionResult::Continue
        }
        KeyCode::Char('v') => {
            app.enter_visual();
            ActionResult::Continue
        }
        KeyCode::Char('e') => {
            if !app.order.is_empty() {
                app.mode = Mode::Detail;
            }
            ActionResult::Continue
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            ActionResult::Continue
        }
        KeyCode::Char('?') => {
            app.mode = Mode::Help;
            ActionResult::Continue
        }
        _ => ActionResult::Continue,
    }
}

pub fn handle_visual_key(app: &mut App, key: KeyEvent) -> ActionResult {
    match key.code {
        KeyCode::Esc | KeyCode::Char('v') => {
            app.mode = Mode::Normal;
            ActionResult::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            ActionResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            ActionResult::Continue
        }
        KeyCode::PageUp => {
            app.move_page_up();
            ActionResult::Continue
        }
        KeyCode::PageDown => {
            app.move_page_down();
            ActionResult::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.move_top();
            ActionResult::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.move_bottom();
            ActionResult::Continue
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_half_page_up();
            ActionResult::Continue
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_half_page_down();
            ActionResult::Continue
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            app.apply_visual_toggle();
            ActionResult::Continue
        }
        KeyCode::Char('a') => {
            app.apply_visual_op(true);
            ActionResult::Continue
        }
        KeyCode::Char('n') => {
            app.apply_visual_op(false);
            ActionResult::Continue
        }
        _ => ActionResult::Continue,
    }
}

pub fn handle_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.filter_query.clear();
            app.apply_filter();
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            app.filter_query.pop();
            app.apply_filter();
        }
        KeyCode::Char(c) => {
            app.filter_query.push(c);
            app.apply_filter();
        }
        _ => {}
    }
}

pub fn handle_help_key(app: &mut App, key: KeyEvent) -> ActionResult {
    if app.help_searching {
        match key.code {
            KeyCode::Esc => {
                app.help_filter.clear();
                app.apply_help_filter();
                app.help_searching = false;
            }
            KeyCode::Enter => {
                app.help_searching = false;
            }
            KeyCode::Backspace => {
                app.help_filter.pop();
                app.apply_help_filter();
            }
            KeyCode::Char(c) => {
                app.help_filter.push(c);
                app.apply_help_filter();
            }
            _ => {}
        }
        return ActionResult::Continue;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
            app.help_filter.clear();
            app.apply_help_filter();
            app.help_searching = false;
            app.mode = Mode::Normal;
            ActionResult::Continue
        }
        KeyCode::Char('/') => {
            app.help_searching = true;
            ActionResult::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.help_move_up();
            ActionResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.help_move_down();
            ActionResult::Continue
        }
        KeyCode::Enter => {
            let display_idx = app.help_state.selected().unwrap_or(0);
            let kb_idx = app.help_order.get(display_idx).copied().unwrap_or(0);
            let action = KEYBINDINGS[kb_idx].action;
            app.help_filter.clear();
            app.apply_help_filter();
            app.help_searching = false;
            app.mode = Mode::Normal;
            if matches!(action, Action::OpenHelp) {
                ActionResult::Continue
            } else {
                app.execute_action(action)
            }
        }
        _ => ActionResult::Continue,
    }
}
