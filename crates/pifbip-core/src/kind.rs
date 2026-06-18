//! File-type classification by extension, falling back to magic bytes (`infer`).

use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico",
];

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "avi", "mov", "flv", "wmv"];

const PDF_EXTENSIONS: &[&str] = &["pdf"];

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "csv", "log", "json", "xml", "yaml", "yml", "html", "css", "js", "py", "sh",
    "conf", "ini", "toml", "rs", "go", "java", "c", "cpp", "h", "hpp", "rb", "php",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Image,
    Video,
    Pdf,
    Text,
    Other,
}

pub fn detect_kind(filepath: &Path) -> FileKind {
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
    if PDF_EXTENSIONS.contains(&ext.as_str()) {
        return FileKind::Pdf;
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
        if mime == "application/pdf" {
            return FileKind::Pdf;
        }
        if mime.starts_with("text/") {
            return FileKind::Text;
        }
        return FileKind::Other;
    }

    FileKind::Other
}
