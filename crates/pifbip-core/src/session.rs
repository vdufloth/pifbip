//! [`SortSession`] — the UI-agnostic file-sorting state machine.
//!
//! Lifted out of the original CLI `main.rs` loop so both front-ends drive the
//! identical engine: preview the current file, choose a destination subfolder
//! (move), skip it, or step back and undo. It performs file moves but no
//! stdout/terminal I/O; results are reported via [`Outcome`].

use std::path::{Path, PathBuf};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::files::{collect_files, get_subdirs, move_file, rename_subdir, resolve_collision};
use crate::kind::{detect_kind, FileKind};

/// What happened to a file, tracked per index so navigation can undo it.
pub enum Action {
    Moved(PathBuf),
    Skipped,
}

/// Snapshot of progress for headers/status lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub index: usize,
    pub total: usize,
    pub moved: usize,
    pub skipped: usize,
}

/// Result of a mutating call, for the front-end to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Moved {
        subfolder: String,
        dest_name: String,
    },
    Skipped,
    Undone,
    MoveError(String),
    AtStart,
    Done,
}

pub struct SortSession {
    destination: PathBuf,
    file_list: Vec<PathBuf>,
    history: Vec<Option<Action>>,
    recent_dirs: Vec<String>,
    index: usize,
    moved: usize,
    skipped: usize,
    matcher: SkimMatcherV2,
}

impl SortSession {
    /// Build a session by scanning `origin` up to `depth` for files.
    pub fn new(origin: &Path, destination: PathBuf, depth: u16) -> Self {
        let file_list = collect_files(origin, depth);
        Self::from_file_list(file_list, destination)
    }

    /// Build a session from a pre-computed file list (used by tests and the GUI).
    pub fn from_file_list(file_list: Vec<PathBuf>, destination: PathBuf) -> Self {
        let total = file_list.len();
        let history = (0..total).map(|_| None).collect();
        Self {
            destination,
            file_list,
            history,
            recent_dirs: Vec::new(),
            index: 0,
            moved: 0,
            skipped: 0,
            matcher: SkimMatcherV2::default(),
        }
    }

    // --- queries -----------------------------------------------------------

    pub fn is_done(&self) -> bool {
        self.index >= self.file_list.len()
    }

    pub fn total(&self) -> usize {
        self.file_list.len()
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn progress(&self) -> Progress {
        Progress {
            index: self.index,
            total: self.file_list.len(),
            moved: self.moved,
            skipped: self.skipped,
        }
    }

    /// Path where the current file currently lives (its moved-to location if it
    /// was moved and we navigated back into it, otherwise its original path).
    pub fn current(&self) -> Option<PathBuf> {
        let original = self.file_list.get(self.index)?;
        match &self.history[self.index] {
            Some(Action::Moved(moved_to)) => Some(moved_to.clone()),
            _ => Some(original.clone()),
        }
    }

    /// Original filename of the current file (for headers/list display).
    pub fn current_original_name(&self) -> Option<String> {
        self.file_list.get(self.index).map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
    }

    pub fn current_kind(&self) -> Option<FileKind> {
        self.current().map(|p| detect_kind(&p))
    }

    /// Index of the file currently being decided.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Original file names in listing order (for a GUI file list).
    pub fn file_names(&self) -> Vec<String> {
        self.file_list
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect()
    }

    /// Full list of destination subfolders: most recently used first, then
    /// alphabetical.
    pub fn subdir_listing(&self) -> Vec<String> {
        get_subdirs(&self.destination, &self.recent_dirs)
    }

    /// Fuzzy-ranked subfolder suggestions for `input` (best first). An empty
    /// input returns the full listing in listing order.
    pub fn subdir_suggestions(&self, input: &str) -> Vec<String> {
        let dirs = self.subdir_listing();
        if input.is_empty() {
            return dirs;
        }
        let mut scored: Vec<(String, i64)> = dirs
            .into_iter()
            .filter_map(|d| self.matcher.fuzzy_match(&d, input).map(|s| (d, s)))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().map(|(d, _)| d).collect()
    }

    // --- mutations ---------------------------------------------------------

    /// Advance past a file that no longer exists, without recording history or
    /// adjusting counts (mirrors the original `!exists -> i += 1; continue`).
    pub fn skip_missing(&mut self) {
        self.index += 1;
    }

    /// Move the current file into `subfolder` under the destination, then advance.
    pub fn move_to(&mut self, subfolder: &str) -> Outcome {
        let Some(source) = self.current() else {
            return Outcome::Done;
        };

        let target_dir = self.destination.join(subfolder);
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            // Original behavior: stay on the same file on dir-creation failure.
            return Outcome::MoveError(format!("creating directory: {}", e));
        }

        let original_name = self.file_list[self.index]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let dest_file = resolve_collision(&target_dir.join(&original_name));

        match move_file(&source, &dest_file) {
            Ok(()) => {
                match &self.history[self.index] {
                    Some(Action::Moved(_)) => {} // re-doing, already counted
                    Some(Action::Skipped) => {
                        self.skipped -= 1;
                        self.moved += 1;
                    }
                    None => self.moved += 1,
                }
                self.history[self.index] = Some(Action::Moved(dest_file.clone()));
                self.recent_dirs.retain(|d| d != subfolder);
                self.recent_dirs.insert(0, subfolder.to_string());
                let dest_name = dest_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                self.index += 1;
                Outcome::Moved {
                    subfolder: subfolder.to_string(),
                    dest_name,
                }
            }
            Err(e) => {
                // Original behavior: advance even on a move error.
                self.index += 1;
                Outcome::MoveError(format!("moving file: {}", e))
            }
        }
    }

    /// Skip the current file (undoing any prior move), then advance.
    pub fn skip(&mut self) -> Outcome {
        let mut outcome = Outcome::Skipped;

        // If this file was previously moved, restore it to its original location.
        if let Some(Action::Moved(moved_path)) = &self.history[self.index] {
            let moved_path = moved_path.clone();
            let original = &self.file_list[self.index];
            if let Err(e) = move_file(&moved_path, original) {
                outcome = Outcome::MoveError(format!("undoing move: {}", e));
            } else {
                self.moved -= 1;
            }
        }

        match &self.history[self.index] {
            Some(Action::Skipped) => {} // already counted
            _ => self.skipped += 1,
        }
        self.history[self.index] = Some(Action::Skipped);
        self.index += 1;
        outcome
    }

    /// Step back to the previous file and undo whatever happened to it.
    pub fn go_back(&mut self) -> Outcome {
        if self.index == 0 {
            return Outcome::AtStart;
        }
        self.index -= 1;
        match &self.history[self.index] {
            Some(Action::Moved(moved_path)) => {
                let moved_path = moved_path.clone();
                let original = &self.file_list[self.index];
                if let Err(e) = move_file(&moved_path, original) {
                    return Outcome::MoveError(format!("undoing move: {}", e));
                }
                self.moved -= 1;
                self.history[self.index] = None;
            }
            Some(Action::Skipped) => {
                self.skipped -= 1;
                self.history[self.index] = None;
            }
            None => {}
        }
        Outcome::Undone
    }

    /// Rename a destination subfolder on disk. Recent-dir bookkeeping is updated
    /// so suggestions stay consistent.
    pub fn rename_subdir(&mut self, old: &str, new: &str) -> std::io::Result<()> {
        rename_subdir(&self.destination, old, new)?;
        if let Some(slot) = self.recent_dirs.iter_mut().find(|d| *d == old) {
            *slot = new.to_string();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create `names` as empty files in `dir`, returning their paths sorted.
    fn make_files(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn move_advances_and_counts() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let files = make_files(src.path(), &["a.txt", "b.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());

        assert_eq!(s.total(), 2);
        let out = s.move_to("photos");
        assert!(matches!(out, Outcome::Moved { .. }));
        assert_eq!(s.progress().moved, 1);
        assert_eq!(s.progress().index, 1);
        assert!(dst.path().join("photos").join("a.txt").exists());
        assert!(!src.path().join("a.txt").exists());
    }

    #[test]
    fn skip_then_back_restores_counts() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let files = make_files(src.path(), &["a.txt", "b.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());

        s.skip();
        assert_eq!(s.progress().skipped, 1);
        assert_eq!(s.progress().index, 1);

        // Go back to file 0 — the skip is undone.
        assert_eq!(s.go_back(), Outcome::Undone);
        assert_eq!(s.progress().skipped, 0);
        assert_eq!(s.progress().index, 0);
    }

    #[test]
    fn move_then_back_restores_file_to_origin() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let files = make_files(src.path(), &["a.txt"]);
        let orig = files[0].clone();
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());

        s.move_to("docs");
        assert!(!orig.exists());
        assert!(s.is_done());

        // Back from the end re-enters file 0 and restores it.
        assert_eq!(s.go_back(), Outcome::Undone);
        assert_eq!(s.progress().moved, 0);
        assert!(orig.exists());
        assert!(!dst.path().join("docs").join("a.txt").exists());
    }

    #[test]
    fn move_after_skip_swaps_counts() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let files = make_files(src.path(), &["a.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());

        s.skip();
        assert_eq!(s.progress().skipped, 1);
        s.go_back();
        // Now re-decide: move it instead.
        s.move_to("keep");
        assert_eq!(s.progress().skipped, 0);
        assert_eq!(s.progress().moved, 1);
    }

    #[test]
    fn collision_resolves_to_suffixed_name() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // Pre-create a colliding file in the target subfolder.
        let sub = dst.path().join("dup");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("a.txt"), b"existing").unwrap();

        let files = make_files(src.path(), &["a.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());
        match s.move_to("dup") {
            Outcome::Moved { dest_name, .. } => assert_eq!(dest_name, "a_1.txt"),
            other => panic!("expected move, got {:?}", other),
        }
    }

    #[test]
    fn go_back_at_start_reports_at_start() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let files = make_files(src.path(), &["a.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());
        assert_eq!(s.go_back(), Outcome::AtStart);
    }

    #[test]
    fn recent_dirs_ordering_drives_suggestions() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        for d in ["alpha", "beta", "gamma"] {
            fs::create_dir_all(dst.path().join(d)).unwrap();
        }
        let files = make_files(src.path(), &["a.txt", "b.txt"]);
        let mut s = SortSession::from_file_list(files, dst.path().to_path_buf());

        s.move_to("gamma");
        // Most recently used folder should lead the listing.
        assert_eq!(
            s.subdir_listing().first().map(String::as_str),
            Some("gamma")
        );
    }
}
