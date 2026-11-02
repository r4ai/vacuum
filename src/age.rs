//! When a project was last worked on (`--older-than`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    number
        .checked_mul(hours * 60 * 60)
        .map(Duration::from_secs)
        .ok_or_else(|| format!("{text:?} is too long"))
}

/// Answers when the project that holds a target last changed.
///
/// A target's project is its parent directory. The project changed when
/// any file under it changed, except files in the target itself: a build
/// refreshes the target without any change to the project.
///
/// Each project is walked once. For every direct child of the project
/// (a target is always one), the index keeps the newest file time in
/// that child. Keeping the two newest children answers "newest except
/// this target" for any target in O(1).
#[derive(Default)]
pub struct AgeIndex {
    projects: HashMap<PathBuf, Newest>,
}

/// The two newest direct children of a project.
#[derive(Default)]
struct Newest {
    first: Option<(PathBuf, SystemTime)>,
    second: Option<SystemTime>,
}

impl Newest {
    fn add(&mut self, child: PathBuf, time: SystemTime) {
        match &self.first {
            Some((_, first)) if time <= *first => {
                self.second = self.second.max(Some(time));
            }
            _ => {
                self.second = self.first.take().map(|(_, t)| t).max(self.second);
                self.first = Some((child, time));
            }
        }
    }

    /// The newest time among the children other than `excluded`.
    fn excluding(&self, excluded: &Path) -> Option<SystemTime> {
        match &self.first {
            Some((child, _)) if child == excluded => self.second,
            Some((_, time)) => Some(*time),
            None => None,
        }
    }
}

impl AgeIndex {
    /// When the project holding `target` last changed; `None` when the
    /// project holds no file outside the target.
    pub fn last_modified(&mut self, target: &Path) -> Option<SystemTime> {
        let project = target.parent()?;
        self.projects
            .entry(project.to_path_buf())
            .or_insert_with_key(|project| scan_project(project))
            .excluding(target)
    }

    /// Whether the project holding `target` last changed more than
    /// `older_than` ago. A project with no other file counts as old.
    pub fn is_older_than(&mut self, target: &Path, older_than: Duration) -> bool {
        let Some(modified) = self.last_modified(target) else {
            return true;
        };
        SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age > older_than)
    }
}

/// The two newest direct children of `project`, by the newest file time
/// in each. Symlinks are not followed.
fn scan_project(project: &Path) -> Newest {
    let mut newest = Newest::default();
    let Ok(children) = std::fs::read_dir(project) else {
        return newest;
    };
    for child in children.flatten() {
        let path = child.path();
        let time = walkdir::WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok()?.modified().ok())
            .max();
        if let Some(time) = time {
            newest.add(path, time);
        }
    }
    newest
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

    /// A file target must not hide the files next to it.
    #[test]
    fn file_target_does_not_skip_its_siblings() {
        let dir = tempfile::TempDir::new().unwrap();
        let old = SystemTime::now() - Duration::from_secs(90 * 86400);
        let target = dir.path().join("a.pyc");
        std::fs::write(&target, "").unwrap();
        for i in 0..50 {
            let file = dir.path().join(format!("src{i}.py"));
            std::fs::write(&file, "").unwrap();
            if i > 0 {
                std::fs::File::options()
                    .write(true)
                    .open(&file)
                    .unwrap()
                    .set_modified(old)
                    .unwrap();
            }
        }
        // Only `src0.py` is new; it must be found wherever it is listed.
        assert!(!AgeIndex::default().is_older_than(&target, Duration::from_secs(86400)));
    }

    #[test]
    fn rejects_overflowing_duration() {
        assert!(parse_duration("4611686018427387904h").is_err());
    }

    /// Other candidates in the same project count as changes; the
    /// candidate itself does not.
    #[test]
    fn each_file_target_excludes_only_itself() {
        let dir = tempfile::TempDir::new().unwrap();
        let old = SystemTime::now() - Duration::from_secs(90 * 86400);
        for name in ["b.pyc", "src.py"] {
            let file = dir.path().join(name);
            std::fs::write(&file, "").unwrap();
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        std::fs::write(dir.path().join("a.pyc"), "").unwrap();

        let mut index = AgeIndex::default();
        let day = Duration::from_secs(86400);
        assert!(index.is_older_than(&dir.path().join("a.pyc"), day));
        assert!(!index.is_older_than(&dir.path().join("b.pyc"), day));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_duration("30").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("3y").is_err());
        assert!(parse_duration("soon").is_err());
    }
}
