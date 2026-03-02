// ─── Sort ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Adapter,
    Path,
    Size,
    Description,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn toggle(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }

    pub fn indicator(self) -> &'static str {
        match self {
            SortDir::Asc => " ↑",
            SortDir::Desc => " ↓",
        }
    }
}

// ─── Mode / Action ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Visual,
    Search,
    Help,
    Detail,
}

#[derive(Clone, Copy)]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveTop,
    MoveBottom,
    MovePageUp,
    MovePageDown,
    MoveHalfPageUp,
    MoveHalfPageDown,
    Toggle,
    SelectAll,
    SelectNone,
    SortByAdapter,
    SortByPath,
    SortBySize,
    SortByDescription,
    EnterVisual,
    OpenDetail,
    OpenSearch,
    OpenHelp,
    Confirm,
    Quit,
}

pub enum ActionResult {
    Continue,
    Confirm,
    Quit,
}

// ─── Keybinding registry ─────────────────────────────────────────────────────

pub struct KeyBinding {
    pub key: &'static str,
    pub desc: &'static str,
    pub action: Action,
}

pub static KEYBINDINGS: &[KeyBinding] = &[
    KeyBinding { key: "↑ / k",    desc: "Move cursor up",              action: Action::MoveUp },
    KeyBinding { key: "↓ / j",    desc: "Move cursor down",            action: Action::MoveDown },
    KeyBinding { key: "PgUp",     desc: "Page up",                     action: Action::MovePageUp },
    KeyBinding { key: "PgDn",     desc: "Page down",                   action: Action::MovePageDown },
    KeyBinding { key: "Ctrl+u",   desc: "Half page up",                action: Action::MoveHalfPageUp },
    KeyBinding { key: "Ctrl+d",   desc: "Half page down",              action: Action::MoveHalfPageDown },
    KeyBinding { key: "g / Home", desc: "Jump to top",                 action: Action::MoveTop },
    KeyBinding { key: "G / End",  desc: "Jump to bottom",              action: Action::MoveBottom },
    KeyBinding { key: "Space",    desc: "Toggle item selection",       action: Action::Toggle },
    KeyBinding { key: "a",        desc: "Select all items",            action: Action::SelectAll },
    KeyBinding { key: "n",        desc: "Deselect all items",          action: Action::SelectNone },
    KeyBinding { key: "v",        desc: "Enter visual selection mode", action: Action::EnterVisual },
    KeyBinding { key: "e",        desc: "Show full details of cursor row", action: Action::OpenDetail },
    KeyBinding { key: "1",        desc: "Sort by Adapter",             action: Action::SortByAdapter },
    KeyBinding { key: "2",        desc: "Sort by Path",                action: Action::SortByPath },
    KeyBinding { key: "3",        desc: "Sort by Size",                action: Action::SortBySize },
    KeyBinding { key: "4",        desc: "Sort by Description",         action: Action::SortByDescription },
    KeyBinding { key: "/",        desc: "Filter items",                action: Action::OpenSearch },
    KeyBinding { key: "?",        desc: "Toggle this help screen",     action: Action::OpenHelp },
    KeyBinding { key: "Enter",    desc: "Confirm and delete",          action: Action::Confirm },
    KeyBinding { key: "q / Esc",  desc: "Quit without deleting",       action: Action::Quit },
];
