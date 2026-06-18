//! PDF first-page rendering (via `pdftoppm`) and external-tool availability probes.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static PDF_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn has_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn has_pdftoppm() -> bool {
    Command::new("pdftoppm")
        .arg("-v")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render the first page of `filepath` to a PNG and return its path.
///
/// The output path is unique per process and call (PID + counter) so concurrent
/// runs don't collide and path-keyed image caches (e.g. iced's) don't serve a
/// stale page. Callers are responsible for deleting the returned file.
pub fn render_pdf_page(filepath: &std::path::Path) -> Option<PathBuf> {
    let id = PDF_COUNTER.fetch_add(1, Ordering::Relaxed);
    let prefix = std::env::temp_dir().join(format!("pifbip-pdf-{}-{}", std::process::id(), id));
    let prefix_str = prefix.to_string_lossy().to_string();

    let output = Command::new("pdftoppm")
        .args(["-png", "-f", "1", "-l", "1", "-r", "150"])
        .arg(filepath)
        .arg(&prefix_str)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            // pdftoppm appends "-1.png" for the single rendered page.
            let png = PathBuf::from(format!("{}-1.png", prefix_str));
            if png.exists() {
                Some(png)
            } else {
                None
            }
        }
        _ => None,
    }
}
