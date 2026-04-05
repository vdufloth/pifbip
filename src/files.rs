use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn collect_files(origin: &Path, max_depth: u16) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(origin, max_depth, 0, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &Path, max_depth: u16, current_depth: u16, result: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_file() {
            result.push(path);
        } else if path.is_dir() && current_depth < max_depth {
            collect_recursive(&path, max_depth, current_depth + 1, result);
        }
    }
}

/// Returns subdirs: most recently used first (latest → oldest), then remaining alphabetically.
/// `recent` vec: index 0 = latest used, index 1 = second latest, etc.
pub fn get_subdirs(destination: &Path, recent: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = fs::read_dir(destination)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    dirs.sort(); // alphabetical base

    // Example: recent = ["sports", "fruits", "instruments"]
    // Result:  ["sports", "fruits", "instruments", ...remaining alphabetical...]
    let mut result: Vec<String> = Vec::with_capacity(dirs.len());
    for r in recent {
        if let Some(pos) = dirs.iter().position(|d| d == r) {
            result.push(dirs.remove(pos));
        }
    }
    result.extend(dirs);
    result
}

pub fn rename_subdir(destination: &Path, old_name: &str, new_name: &str) -> io::Result<()> {
    let old_path = destination.join(old_name);
    let new_path = destination.join(new_name);
    fs::rename(old_path, new_path)
}

pub fn resolve_collision(dest_file: &Path) -> PathBuf {
    if !dest_file.exists() {
        return dest_file.to_path_buf();
    }

    let stem = dest_file
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = dest_file
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = dest_file.parent().unwrap_or(Path::new("."));

    let mut counter = 1u32;
    loop {
        let candidate = parent.join(format!("{}_{}{}", stem, counter, ext));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Cross-device move: fall back to copy + delete
            if e.raw_os_error() == Some(18) {
                fs::copy(src, dst)?;
                fs::remove_file(src)?;
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

pub fn format_size(size: u64) -> String {
    let mut size = size as f64;
    for unit in &["B", "KB", "MB", "GB"] {
        if size < 1024.0 {
            if *unit == "B" {
                return format!("{} {}", size as u64, unit);
            }
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} TB", size)
}
