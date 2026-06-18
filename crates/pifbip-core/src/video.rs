//! Raw video-frame pipeline: stream rgb24 frames out of `ffmpeg`.
//!
//! Returns frames as raw rgb24 bytes (`w * h * 3`) so the crate stays UI-agnostic.
//! The CLI packs these into a `u32` minifb buffer; the GUI expands them to rgba
//! for an iced image handle.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub struct FrameSize {
    pub w: usize,
    pub h: usize,
}

impl FrameSize {
    pub fn frame_bytes(&self) -> usize {
        self.w * self.h * 3
    }
}

/// Spawn ffmpeg decoding `path` to a raw rgb24 stream scaled to `size`.
pub fn spawn_ffmpeg(path: &Path, size: FrameSize) -> Option<Child> {
    Command::new("ffmpeg")
        .args([
            "-i",
            &path.to_string_lossy(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            &format!("{}x{}", size.w, size.h),
            "-v",
            "quiet",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Read exactly one rgb24 frame (`size.frame_bytes()` bytes) from the ffmpeg
/// child's stdout. Returns `None` on EOF (video ended) or error.
///
/// This blocks until a full frame is read — never call it on an async executor.
pub fn read_frame_rgb(child: &mut Child, size: FrameSize) -> Option<Vec<u8>> {
    let stdout = child.stdout.as_mut()?;
    let frame_bytes = size.frame_bytes();
    let mut rgb_buf = vec![0u8; frame_bytes];

    let mut read = 0;
    while read < frame_bytes {
        match stdout.read(&mut rgb_buf[read..]) {
            Ok(0) => return None, // EOF — video ended
            Ok(n) => read += n,
            Err(_) => return None,
        }
    }

    Some(rgb_buf)
}
