//! Data that AI coding agents leave on disk: session logs, caches and
//! snapshots.
//!
//! Most agents keep this data in a home directory such as `~/.claude`,
//! so these adapters find it when the scan reaches that directory
//! (e.g. `vacuum ~` or `vacuum ~/.claude`). Some of it is conversation
//! history, so every agent adapter is off by default.

use std::path::Path;

use crate::adapter::{Adapter, Context};

/// One kind of agent data: a directory name and what it holds.
#[derive(Clone, Copy)]
pub struct Data {
    pub dir: &'static str,
    pub what: &'static str,
}

/// An agent that keeps its data in a home directory.
#[derive(Clone, Copy)]
pub struct AgentHome {
    pub name: &'static str,
    /// Product name shown in descriptions, e.g. `"Claude Code"`.
    pub label: &'static str,
    /// Home directories as trailing path components, e.g.
    /// `[".gemini", "antigravity"]` for `~/.gemini/antigravity`.
    pub homes: &'static [&'static [&'static str]],
    /// Entries of which the home must hold at least one. They tell the
    /// home apart from a project directory of the same name, such as a
    /// repository's `.claude/`. Empty when the name alone is enough.
    pub markers: &'static [&'static str],
    pub data: &'static [Data],
}

impl AgentHome {
    fn is_home(&self, dir: &Path, ctx: &mut Context) -> bool {
        self.homes.iter().any(|home| ends_with(dir, home))
            && (self.markers.is_empty() || self.markers.iter().any(|m| ctx.has_file(dir, m)))
    }
}

/// Whether the last components of `path` are `components`.
fn ends_with(path: &Path, components: &[&str]) -> bool {
    let mut names = path.iter().rev();
    components
        .iter()
        .rev()
        .all(|expected| names.next().is_some_and(|name| name == *expected))
}

impl Adapter for AgentHome {
    fn name(&self) -> &'static str {
        self.name
    }

    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        // The name check comes first so that unrelated directories cost
        // no I/O.
        let data = self.data.iter().find(|d| d.dir == name)?;
        self.is_home(parent, ctx)
            .then(|| format!("{} {} ({name}/)", self.label, data.what))
    }
}

/// Claude Code (`~/.claude`). Transcripts under `projects/` are kept:
/// they back `--resume` and hold the auto memory, and Claude Code
/// already removes old ones (`cleanupPeriodDays`).
pub static CLAUDE_CODE: AgentHome = AgentHome {
    name: "claude-code",
    label: "Claude Code",
    homes: &[&[".claude"]],
    // A repository's `.claude/` has none of these.
    markers: &["projects", "history.jsonl", ".credentials.json"],
    data: &[
        Data {
            dir: "debug",
            what: "debug logs",
        },
        Data {
            dir: "shell-snapshots",
            what: "shell snapshots",
        },
        Data {
            dir: "file-history",
            what: "checkpoints for rewind",
        },
        Data {
            dir: "paste-cache",
            what: "paste cache",
        },
        Data {
            dir: "image-cache",
            what: "image cache",
        },
        Data {
            dir: "session-env",
            what: "session environments",
        },
        Data {
            dir: "todos",
            what: "legacy todo lists",
        },
        Data {
            dir: "statsig",
            what: "legacy feature flag cache",
        },
        Data {
            dir: "logs",
            what: "legacy logs",
        },
    ],
};

/// OpenAI Codex (`~/.codex`). Managed worktrees under `worktrees/` are
/// kept: they may hold uncommitted work.
pub static CODEX: AgentHome = AgentHome {
    name: "codex",
    label: "Codex",
    homes: &[&[".codex"]],
    // A repository's `.codex/` has none of these.
    markers: &["auth.json", "history.jsonl", "sessions", "version.json"],
    data: &[
        Data {
            dir: "sessions",
            what: "session history",
        },
        Data {
            dir: "archived_sessions",
            what: "archived session history",
        },
        Data {
            dir: "log",
            what: "logs",
        },
        Data {
            dir: "shell_snapshots",
            what: "shell snapshots",
        },
    ],
};

/// Google Antigravity, both the IDE (`~/.gemini/antigravity`) and the
/// CLI (`~/.gemini/antigravity-cli`).
pub static ANTIGRAVITY: AgentHome = AgentHome {
    name: "antigravity",
    label: "Antigravity",
    homes: &[&[".gemini", "antigravity"], &[".gemini", "antigravity-cli"]],
    markers: &[],
    data: &[
        Data {
            dir: "browser_recordings",
            what: "browser recordings",
        },
        Data {
            dir: "brain",
            what: "conversation artifacts and logs",
        },
        Data {
            dir: "conversations",
            what: "conversation history",
        },
        Data {
            dir: "implicit",
            what: "implicit context",
        },
        Data {
            dir: "code_tracker",
            what: "code tracker state",
        },
    ],
};

/// Gemini CLI (`~/.gemini/tmp`): per-project chats, checkpoints and logs.
pub static GEMINI_CLI: AgentHome = AgentHome {
    name: "gemini-cli",
    label: "Gemini CLI",
    homes: &[&[".gemini"]],
    markers: &[],
    data: &[Data {
        dir: "tmp",
        what: "chats, checkpoints and logs",
    }],
};

/// Aider, which writes its history and repo map cache into the project.
pub struct AiderAdapter;

impl AiderAdapter {
    const FILES: &[(&str, &str)] = &[
        (".aider.chat.history.md", "chat history"),
        (".aider.input.history", "input history"),
        (".aider.llm.history", "LLM log"),
    ];
    /// Followed by a format version, e.g. `.aider.tags.cache.v4`.
    const TAGS_CACHE: &str = ".aider.tags.cache.v";
}

impl Adapter for AiderAdapter {
    fn name(&self) -> &'static str {
        "aider"
    }

    fn match_dir(&self, name: &str, _parent: &Path, _ctx: &mut Context) -> Option<String> {
        name.strip_prefix(Self::TAGS_CACHE)
            .is_some_and(|version| {
                !version.is_empty() && version.chars().all(|c| c.is_ascii_digit())
            })
            .then(|| format!("Aider repo map cache ({name}/)"))
    }

    fn match_file(&self, name: &str, _parent: &Path, _ctx: &mut Context) -> Option<String> {
        Self::FILES
            .iter()
            .find(|(file, _)| *file == name)
            .map(|(_, what)| format!("Aider {what} ({name})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_with;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ends_with_compares_trailing_components() {
        let path = Path::new("/home/u/.gemini/antigravity");
        assert!(ends_with(path, &["antigravity"]));
        assert!(ends_with(path, &[".gemini", "antigravity"]));
        assert!(!ends_with(path, &[".gemini"]));
        assert!(!ends_with(
            Path::new("antigravity"),
            &[".gemini", "antigravity"]
        ));
    }

    #[test]
    fn description_names_the_agent_and_the_data() {
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        fs::create_dir_all(claude.join("projects")).unwrap();
        fs::create_dir_all(claude.join("debug")).unwrap();

        let targets = scan_with(CLAUDE_CODE, dir.path());

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].description, "Claude Code debug logs (debug/)");
    }

    #[test]
    fn aider_tags_cache_needs_a_version() {
        let dir = TempDir::new().unwrap();
        for name in [
            ".aider.tags.cache.v3",
            ".aider.tags.cache.v",
            ".aider.tags.cache.vx",
        ] {
            fs::create_dir_all(dir.path().join(name)).unwrap();
        }

        let targets = scan_with(AiderAdapter, dir.path());

        let names: Vec<_> = targets
            .iter()
            .map(|t| t.path.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, [".aider.tags.cache.v3"]);
    }
}
