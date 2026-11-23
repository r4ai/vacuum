//! Paths a scan could not read, such as directories without permission.
//! They are reported at the end instead of being dropped silently.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The paths a scan could not read, each with its first error.
#[derive(Debug, Default)]
pub struct SkippedPaths(BTreeMap<PathBuf, String>);

impl SkippedPaths {
    /// Record that `path` could not be read. Several walks may fail on
    /// the same path; only the first error is kept.
    pub fn add(&mut self, path: PathBuf, error: String) {
        self.0.entry(path).or_insert(error);
    }

    /// Record an error of the `ignore` walker. `fallback` is used when the
    /// error names no path.
    pub fn add_ignore_error(&mut self, error: ignore::Error, fallback: &Path) {
        match error {
            ignore::Error::WithPath { path, err } => self.add_at(*err, &path),
            ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
                self.add_ignore_error(*err, fallback)
            }
            ignore::Error::Partial(errors) => {
                for error in errors {
                    self.add_ignore_error(error, fallback);
                }
            }
            other => self.add(fallback.to_path_buf(), other.to_string()),
        }
    }

    /// Like [`Self::add_ignore_error`], but the path is known.
    fn add_at(&mut self, error: ignore::Error, path: &Path) {
        match error {
            ignore::Error::WithPath { .. }
            | ignore::Error::WithDepth { .. }
            | ignore::Error::WithLineNumber { .. }
            | ignore::Error::Partial(_) => self.add_ignore_error(error, path),
            other => self.add(path.to_path_buf(), other.to_string()),
        }
    }

    /// Record an error of the `walkdir` walker.
    pub fn add_walkdir_error(&mut self, error: walkdir::Error, fallback: &Path) {
        let path = error.path().unwrap_or(fallback).to_path_buf();
        let message = match error.io_error() {
            Some(io) => io.to_string(),
            None => error.to_string(),
        };
        self.add(path, message);
    }

    pub fn extend(&mut self, other: SkippedPaths) {
        for (path, error) in other.0 {
            self.add(path, error);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `(path, error)` in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.0.iter().map(|(p, e)| (p.as_path(), e.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn denied() -> io::Error {
        io::Error::from(io::ErrorKind::PermissionDenied)
    }

    #[test]
    fn a_path_is_reported_once_with_its_first_error() {
        let mut skipped = SkippedPaths::default();
        skipped.add("/a".into(), "first".into());
        skipped.add("/a".into(), "second".into());
        skipped.add("/b".into(), "other".into());

        let all: Vec<_> = skipped.iter().collect();
        assert_eq!(
            all,
            [(Path::new("/a"), "first"), (Path::new("/b"), "other")]
        );
    }

    #[test]
    fn ignore_errors_are_reported_at_their_path() {
        let mut skipped = SkippedPaths::default();
        let error = ignore::Error::WithDepth {
            depth: 2,
            err: Box::new(ignore::Error::WithPath {
                path: "/root/locked".into(),
                err: Box::new(ignore::Error::Io(denied())),
            }),
        };

        skipped.add_ignore_error(error, Path::new("/root"));

        let all: Vec<_> = skipped.iter().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, Path::new("/root/locked"));
        assert_eq!(all[0].1, denied().to_string(), "the path is not repeated");
    }

    #[test]
    fn ignore_errors_without_a_path_fall_back() {
        let mut skipped = SkippedPaths::default();
        skipped.add_ignore_error(ignore::Error::Io(denied()), Path::new("/root"));
        assert_eq!(skipped.iter().next().unwrap().0, Path::new("/root"));
    }

    #[test]
    fn extend_keeps_one_entry_per_path() {
        let mut a = SkippedPaths::default();
        a.add("/x".into(), "a".into());
        let mut b = SkippedPaths::default();
        b.add("/x".into(), "b".into());
        b.add("/y".into(), "b".into());

        a.extend(b);

        assert_eq!(a.len(), 2);
        assert_eq!(a.iter().next().unwrap().1, "a");
    }
}
