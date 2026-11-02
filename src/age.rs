//! When a project was last worked on (`--older-than`).

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Parse a duration such as `30d`, `12h` or `2w`.
pub fn parse_duration(text: &str) -> Result<Duration, String> {
    let unit_at = text
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| format!("missing unit in {text:?} (use h, d or w)"))?;
    let (number, unit) = text.split_at(unit_at);
    let number: u64 = number
        .parse()
        .map_err(|_| format!("missing number in {text:?}"))?;
    let hours = match unit {
        "h" => 1,
        "d" => 24,
        "w" => 24 * 7,
        _ => return Err(format!("unknown unit {unit:?} (use h, d or w)")),
    };
    Ok(Duration::from_secs(number * hours * 60 * 60))
}

/// The newest modification time of a file in the project that holds
/// `target`, i.e. under the target's parent directory. The target itself
/// is skipped, since a build refreshes it without any change to the
/// project. `None` when the project holds no other file.
pub fn project_last_modified(target: &Path) -> Option<SystemTime> {
    let project = target.parent()?;
    let mut newest = None;
    let mut walk = walkdir::WalkDir::new(project)
        .follow_links(false)
        .into_iter();
    while let Some(entry) = walk.next() {
        let Ok(entry) = entry else {
            continue;
        };
        if entry.path() == target {
            walk.skip_current_dir();
            continue;
        }
        if entry.file_type().is_file()
            && let Some(modified) = entry.metadata().ok().and_then(|m| m.modified().ok())
        {
            newest = newest.max(Some(modified));
        }
    }
    newest
}

/// Whether the project holding `target` was last modified before
/// `older_than` ago. A project with no other file counts as old.
pub fn is_older_than(target: &Path, older_than: Duration) -> bool {
    let Some(modified) = project_last_modified(target) else {
        return true;
    };
    SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age > older_than)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_units() {
        assert_eq!(parse_duration("12h"), Ok(Duration::from_secs(12 * 3600)));
        assert_eq!(parse_duration("30d"), Ok(Duration::from_secs(30 * 86400)));
        assert_eq!(parse_duration("2w"), Ok(Duration::from_secs(14 * 86400)));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_duration("30").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("3y").is_err());
        assert!(parse_duration("soon").is_err());
    }
}
