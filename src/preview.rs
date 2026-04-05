use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Command;

use crate::files::format_size;
use crate::viewer::PreviewWindow;

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "csv", "log", "json", "xml", "yaml", "yml",
    "html", "css", "js", "py", "sh", "conf", "ini", "toml",
    "rs", "go", "java", "c", "cpp", "h", "hpp", "rb", "php",
];

const HEAD_LINES: usize = 10;
const TAIL_LINES: usize = 10;
const MAX_DISPLAY_LINES: usize = HEAD_LINES + TAIL_LINES + 5;
const MAX_READ_BYTES: u64 = 64 * 1024;

pub enum ImageMode {
    Chafa,
    Viuer,
    Windowed,
}

pub fn has_chafa() -> bool {
    Command::new("chafa")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn has_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

enum FileKind {
    Image,
    Video,
    Text,
    Other,
}

fn detect_kind(filepath: &Path) -> FileKind {
    let ext = filepath
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Try extension first
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return FileKind::Image;
    }
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        return FileKind::Video;
    }
    if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        return FileKind::Text;
    }

    // If extension is unknown or missing, try magic bytes
    if let Ok(Some(kind)) = infer::get_from_path(filepath) {
        let mime = kind.mime_type();
        if mime.starts_with("image/") {
            return FileKind::Image;
        }
        if mime.starts_with("video/") {
            return FileKind::Video;
        }
        if mime.starts_with("text/") {
            return FileKind::Text;
        }
        return FileKind::Other;
    }

    FileKind::Other
}

pub fn show_preview(filepath: &Path, image_mode: &ImageMode, viewer: Option<&PreviewWindow>) {
    let kind = detect_kind(filepath);

    match kind {
        FileKind::Image => preview_image(filepath, image_mode, viewer),
        FileKind::Video => preview_video(filepath, image_mode, viewer),
        FileKind::Text => {
            if matches!(image_mode, ImageMode::Windowed) {
                if let Some(v) = viewer {
                    v.clear();
                }
            }
            preview_text(filepath);
        }
        FileKind::Other => {
            if matches!(image_mode, ImageMode::Windowed) {
                if let Some(v) = viewer {
                    v.clear();
                }
            }
            preview_other(filepath);
        }
    }
}

fn preview_image(filepath: &Path, image_mode: &ImageMode, viewer: Option<&PreviewWindow>) {
    let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));

    let reserved = 17u16;
    let img_height = if term_height > reserved {
        term_height - reserved
    } else {
        8
    };

    match image_mode {
        ImageMode::Windowed => {
            if let Some(v) = viewer {
                v.show_image(filepath);
                println!("  (preview in window)");
            } else {
                preview_other(filepath);
            }
        }
        ImageMode::Chafa => {
            if !preview_image_chafa(filepath, term_width, img_height) {
                preview_other(filepath);
            }
        }
        ImageMode::Viuer => {
            let conf = viuer::Config {
                width: None,
                height: Some(img_height as u32),
                absolute_offset: false,
                ..Default::default()
            };
            if viuer::print_from_file(filepath, &conf).is_err() {
                preview_other(filepath);
            }
        }
    }
}

fn preview_video(filepath: &Path, image_mode: &ImageMode, viewer: Option<&PreviewWindow>) {
    match image_mode {
        ImageMode::Windowed => {
            if let Some(v) = viewer {
                v.play_video(filepath);
                println!("  (video playing in window)");
            } else {
                preview_other(filepath);
            }
        }
        _ => {
            preview_other(filepath);
        }
    }
}

fn preview_image_chafa(filepath: &Path, width: u16, height: u16) -> bool {
    match Command::new("chafa")
        .arg("--size")
        .arg(format!("{}x{}", width, height))
        .arg("--animate=off")
        .arg(filepath)
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            print!("{}", text);
            true
        }
        _ => false,
    }
}

fn preview_text(filepath: &Path) {
    let cols = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);

    let file = match fs::File::open(filepath) {
        Ok(f) => f,
        Err(e) => {
            println!("  (cannot read file: {})", e);
            return;
        }
    };

    let reader = BufReader::new(file.take(MAX_READ_BYTES));
    let lines: Vec<String> = reader.lines().map(|l| l.unwrap_or_default()).collect();
    let total = lines.len();

    println!(
        "  --- {} line{} ---",
        total,
        if total == 1 { "" } else { "s" }
    );

    if total <= MAX_DISPLAY_LINES {
        for line in &lines {
            print_truncated(line, cols);
        }
    } else {
        for line in &lines[..HEAD_LINES] {
            print_truncated(line, cols);
        }
        println!(
            "  ... ({} lines omitted) ...",
            total - HEAD_LINES - TAIL_LINES
        );
        for line in &lines[total - TAIL_LINES..] {
            print_truncated(line, cols);
        }
    }

    println!("  --- end ---");
}

fn preview_other(filepath: &Path) {
    let size = filepath.metadata().map(|m| m.len()).unwrap_or(0);
    let mime = mime_guess::from_path(filepath)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "unknown type".to_string());
    println!("  Type: {} | Size: {}", mime, format_size(size));
}

fn print_truncated(line: &str, cols: usize) {
    let max = if cols > 4 { cols - 4 } else { cols };
    let truncated: String = line.chars().take(max).collect();
    println!("  {}", truncated);
}
