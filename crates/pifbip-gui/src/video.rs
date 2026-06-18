//! Background video decoding for the preview pane.
//!
//! A dedicated OS thread runs the blocking ffmpeg frame pipeline from
//! `pifbip-core` and stashes the most recent rgb24 frame behind a mutex. The
//! iced app polls it on a timer (never blocking the UI thread) and converts the
//! latest frame to an image handle. Dropping the stream signals the thread to
//! kill ffmpeg and exit.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use pifbip_core::{read_frame_rgb, spawn_ffmpeg, FrameSize};

pub const VIDEO_W: usize = 800;
pub const VIDEO_H: usize = 600;
const SIZE: FrameSize = FrameSize {
    w: VIDEO_W,
    h: VIDEO_H,
};

pub struct VideoStream {
    latest: Arc<Mutex<Option<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
}

impl VideoStream {
    /// Start decoding `path`, looping the video until dropped.
    pub fn start(path: PathBuf) -> Self {
        let latest: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let latest_w = latest.clone();
        let stop_w = stop.clone();

        thread::spawn(move || {
            let mut child = match spawn_ffmpeg(&path, SIZE) {
                Some(c) => c,
                None => return,
            };
            loop {
                if stop_w.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                match read_frame_rgb(&mut child, SIZE) {
                    Some(frame) => {
                        if let Ok(mut slot) = latest_w.lock() {
                            *slot = Some(frame);
                        }
                    }
                    None => {
                        // EOF — loop the video by respawning ffmpeg.
                        let _ = child.kill();
                        let _ = child.wait();
                        if stop_w.load(Ordering::Relaxed) {
                            return;
                        }
                        match spawn_ffmpeg(&path, SIZE) {
                            Some(c) => child = c,
                            None => return,
                        }
                        continue;
                    }
                }
                thread::sleep(Duration::from_millis(33));
            }
        });

        Self { latest, stop }
    }

    /// Take the most recent decoded frame (rgb24), if a new one is available.
    pub fn take_frame(&self) -> Option<Vec<u8>> {
        self.latest.lock().ok().and_then(|mut slot| slot.take())
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        // Signal the worker to stop; it kills ffmpeg and exits after its next
        // frame read. Detached (no join) so navigation never blocks the UI.
        self.stop.store(true, Ordering::Relaxed);
    }
}
