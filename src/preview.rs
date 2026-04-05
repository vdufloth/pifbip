use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::files::format_size;

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico",
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

pub fn show_preview(filepath: &Path) {
    let ext = filepath
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        preview_image(filepath);
    } else if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        preview_text(filepath);
    } else {
        preview_other(filepath);
    }
}

fn preview_image(filepath: &Path) {
    let conf = viuer::Config {
        width: Some(80),
        height: Some(24),
        absolute_offset: false,
        ..Default::default()
    };

    if viuer::print_from_file(filepath, &conf).is_err() {
        preview_other(filepath);
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
