//! Shared core for pifbip — the file-sorting engine and media pipelines used by
//! both `pifbip-cli` (terminal) and `pifbip-gui` (iced desktop) front-ends.
//!
//! This crate carries no UI dependencies: it owns file operations, file-type
//! detection, the PDF/video preview pipelines (shelling out to `pdftoppm` /
//! `ffmpeg`), and the [`SortSession`] state machine that drives the
//! preview → choose-folder → move/skip/undo workflow.

pub mod files;
pub mod kind;
pub mod pdf;
pub mod session;
pub mod video;

pub use files::{
    collect_files, format_size, get_subdirs, move_file, rename_subdir, resolve_collision,
};
pub use kind::{detect_kind, FileKind};
pub use pdf::{has_ffmpeg, has_pdftoppm, render_pdf_page};
pub use session::{Action, Outcome, Progress, SortSession};
pub use video::{read_frame_rgb, spawn_ffmpeg, FrameSize};
