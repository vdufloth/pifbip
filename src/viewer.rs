use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

use image::GenericImageView;
use minifb::{Window, WindowOptions};

const WINDOW_WIDTH: usize = 800;
const WINDOW_HEIGHT: usize = 600;
const FRAME_BYTES: usize = WINDOW_WIDTH * WINDOW_HEIGHT * 3;

enum ViewerMsg {
    Show(Vec<u32>, usize, usize),
    PlayVideo(PathBuf),
    Clear,
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

        let buffer = image_to_buffer(&img);
        let _ = self.tx.send(ViewerMsg::Show(buffer, WINDOW_WIDTH, WINDOW_HEIGHT));
    }

    pub fn play_video(&self, filepath: &Path) {
        let _ = self.tx.send(ViewerMsg::PlayVideo(filepath.to_path_buf()));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(ViewerMsg::Clear);
    }
}

impl Drop for PreviewWindow {
    fn drop(&mut self) {
        let _ = self.tx.send(ViewerMsg::Close);
    }
}

fn image_to_buffer(img: &image::DynamicImage) -> Vec<u32> {
    let (iw, ih) = img.dimensions();
    let scale = f64::min(
        WINDOW_WIDTH as f64 / iw as f64,
        WINDOW_HEIGHT as f64 / ih as f64,
    )
    .min(1.0);

    let new_w = (iw as f64 * scale) as u32;
    let new_h = (ih as f64 * scale) as u32;

    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

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

    buffer
}

fn kill_ffmpeg(proc: &mut Option<Child>) {
    if let Some(mut child) = proc.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn_ffmpeg(path: &Path) -> Option<Child> {
    Command::new("ffmpeg")
        .args([
            "-i", &path.to_string_lossy(),
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-s", &format!("{}x{}", WINDOW_WIDTH, WINDOW_HEIGHT),
            "-v", "quiet",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn read_frame(child: &mut Child) -> Option<Vec<u32>> {
    let stdout = child.stdout.as_mut()?;
    let mut rgb_buf = vec![0u8; FRAME_BYTES];

    let mut read = 0;
    while read < FRAME_BYTES {
        match stdout.read(&mut rgb_buf[read..]) {
            Ok(0) => return None, // EOF — video ended
            Ok(n) => read += n,
            Err(_) => return None,
        }
    }

    let mut buffer = vec![0u32; WINDOW_WIDTH * WINDOW_HEIGHT];
    for i in 0..buffer.len() {
        let r = rgb_buf[i * 3] as u32;
        let g = rgb_buf[i * 3 + 1] as u32;
        let b = rgb_buf[i * 3 + 2] as u32;
        buffer[i] = (r << 16) | (g << 8) | b;
    }

    Some(buffer)
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
    let mut ffmpeg_proc: Option<Child> = None;
    let mut video_path: Option<PathBuf> = None;

    while window.is_open() {
        // Check for new messages
        match rx.try_recv() {
            Ok(ViewerMsg::Show(new_buffer, _w, _h)) => {
                kill_ffmpeg(&mut ffmpeg_proc);
                video_path = None;
                buffer = new_buffer;
            }
            Ok(ViewerMsg::PlayVideo(path)) => {
                kill_ffmpeg(&mut ffmpeg_proc);
                ffmpeg_proc = spawn_ffmpeg(&path);
                video_path = Some(path);
                buffer = vec![0u32; WINDOW_WIDTH * WINDOW_HEIGHT];
            }
            Ok(ViewerMsg::Clear) => {
                kill_ffmpeg(&mut ffmpeg_proc);
                video_path = None;
                buffer = vec![0u32; WINDOW_WIDTH * WINDOW_HEIGHT];
            }
            Ok(ViewerMsg::Close) => {
                kill_ffmpeg(&mut ffmpeg_proc);
                break;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                kill_ffmpeg(&mut ffmpeg_proc);
                break;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // If playing video, try to read next frame
        if let Some(ref mut child) = ffmpeg_proc {
            match read_frame(child) {
                Some(frame) => {
                    buffer = frame;
                }
                None => {
                    // Video ended — loop by respawning ffmpeg
                    kill_ffmpeg(&mut ffmpeg_proc);
                    if let Some(ref path) = video_path {
                        ffmpeg_proc = spawn_ffmpeg(path);
                    }
                }
            }
        }

        let _ = window.update_with_buffer(&buffer, WINDOW_WIDTH, WINDOW_HEIGHT);
    }

    kill_ffmpeg(&mut ffmpeg_proc);
}
