//! Machine-readable output (`--json`).

use serde::Serialize;

use crate::adapter::CleanTarget;
use crate::cleaner::{DeleteMethod, DeleteReport};

#[derive(Serialize)]
struct Report<'a> {
    targets: Vec<Target<'a>>,
    /// Sum of the targets' sizes in bytes.
    total_size: u64,
    result: Outcome<'a>,
}

#[derive(Serialize)]
struct Target<'a> {
    /// Absolute path. Non-UTF-8 bytes are replaced.
    path: String,
    adapter: &'a str,
    description: &'a str,
    /// Size in bytes; 0 with `--no-size`.
    size: u64,
    /// Whether the target contains files tracked by git.
    tracked: bool,
}

#[derive(Serialize)]
struct Outcome<'a> {
    dry_run: bool,
    /// `"remove"` (permanently) or `"trash"` (`--trash`).
    method: &'static str,
    /// Bytes deleted permanently; 0 on a dry run or with `--trash`.
    freed: u64,
    /// Bytes moved to the trash; they still occupy disk space.
    trashed: u64,
    errors: Vec<Error<'a>>,
}

#[derive(Serialize)]
struct Error<'a> {
    path: &'a str,
    error: &'a str,
}

/// The JSON document for `targets` and the outcome of deleting them.
pub fn to_json(
    targets: &[CleanTarget],
    outcome: &DeleteReport,
    dry_run: bool,
    method: DeleteMethod,
) -> String {
    let (method_name, freed, trashed) = match method {
        DeleteMethod::Remove => ("remove", outcome.freed, 0),
        DeleteMethod::Trash => ("trash", 0, outcome.freed),
    };
    let report = Report {
        targets: targets
            .iter()
            .map(|t| Target {
                path: t.path.display().to_string(),
                adapter: t.adapter,
                description: &t.description,
                size: t.size,
                tracked: t.tracked,
            })
            .collect(),
        total_size: targets.iter().map(|t| t.size).sum(),
        result: Outcome {
            dry_run,
            method: method_name,
            freed,
            trashed,
            errors: outcome
                .errors
                .iter()
                .map(|(path, error)| Error { path, error })
                .collect(),
        },
    };
    serde_json::to_string_pretty(&report).expect("the report always serializes")
}
