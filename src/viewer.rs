use std::path::Path;
use std::sync::mpsc;
use std::thread;

use image::GenericImageView;
use minifb::{Window, WindowOptions};

const WINDOW_WIDTH: usize = 800;
const WINDOW_HEIGHT: usize = 600;

enum ViewerMsg {
    Show(Vec<u32>, usize, usize),
    Close,
}

pub struct PreviewWindow {
    tx: mpsc::Sender<ViewerMsg>,
}

impl PreviewWindow {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_window(rx));
        Self { tx }
    }

    pub fn show_image(&self, filepath: &Path) {
        let img = match image::open(filepath) {
            Ok(img) => img,
            Err(_) => return,
        };

        // Resize to fit window while preserving aspect ratio
        let (iw, ih) = img.dimensions();
        let scale = f64::min(
            WINDOW_WIDTH as f64 / iw as f64,
            WINDOW_HEIGHT as f64 / ih as f64,
        )
        .min(1.0); // don't upscale

        let new_w = (iw as f64 * scale) as u32;
        let new_h = (ih as f64 * scale) as u32;

        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

        // Build ARGB u32 buffer padded to WINDOW_WIDTH x WINDOW_HEIGHT (black background)
        let mut buffer = vec![0u32; WINDOW_WIDTH * WINDOW_HEIGHT];
        let offset_x = (WINDOW_WIDTH - new_w as usize) / 2;
        let offset_y = (WINDOW_HEIGHT - new_h as usize) / 2;

        for (x, y, pixel) in resized.pixels() {
            let [r, g, b, _a] = pixel.0;
            let idx = (offset_y + y as usize) * WINDOW_WIDTH + (offset_x + x as usize);
            if idx < buffer.len() {
                buffer[idx] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            }
        }

        let _ = self.tx.send(ViewerMsg::Show(buffer, WINDOW_WIDTH, WINDOW_HEIGHT));
    }
}

impl Drop for PreviewWindow {
    fn drop(&mut self) {
        let _ = self.tx.send(ViewerMsg::Close);
    }
}

fn run_window(rx: mpsc::Receiver<ViewerMsg>) {
    // Force X11 backend via XWayland to avoid Wayland decoration issues with minifb
    std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    std::env::set_var("WAYLAND_DISPLAY", "");

    let mut window = match Window::new(
        "pifbip preview",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        WindowOptions {
            topmost: true,
            ..Default::default()
        },
    ) {
        Ok(w) => w,
        Err(_) => return,
    };

    window.set_target_fps(30);

    let mut buffer = vec![0u32; WINDOW_WIDTH * WINDOW_HEIGHT];

    while window.is_open() {
        match rx.try_recv() {
            Ok(ViewerMsg::Show(new_buffer, _w, _h)) => {
                buffer = new_buffer;
            }
            Ok(ViewerMsg::Close) => break,
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        let _ = window.update_with_buffer(&buffer, WINDOW_WIDTH, WINDOW_HEIGHT);
    }
}
