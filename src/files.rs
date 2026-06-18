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
        .filter(|name| !name.starts_with('.'))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("pifbip-{name}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"test").unwrap();
    }

    fn rel_paths(root: &Path, files: Vec<PathBuf>) -> Vec<String> {
        files
            .into_iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn collect_files_respects_depth_and_ignores_hidden_entries() {
        let tmp = TestDir::new("collect-files");
        let root = tmp.path();

        write_file(&root.join("top.txt"));
        write_file(&root.join(".hidden.txt"));
        write_file(&root.join("sub/one.txt"));
        write_file(&root.join("sub/deeper/two.txt"));
        write_file(&root.join(".hidden-dir/secret.txt"));

        assert_eq!(rel_paths(root, collect_files(root, 0)), vec!["top.txt"]);
        assert_eq!(
            rel_paths(root, collect_files(root, 1)),
            vec!["sub/one.txt", "top.txt"]
        );
        assert_eq!(
            rel_paths(root, collect_files(root, 2)),
            vec!["sub/deeper/two.txt", "sub/one.txt", "top.txt"]
        );
    }

    #[test]
    fn get_subdirs_prioritizes_recent_dirs_then_remaining_alphabetically() {
        let tmp = TestDir::new("get-subdirs");
        let root = tmp.path();

        fs::create_dir_all(root.join("documents")).unwrap();
        fs::create_dir_all(root.join("fruits")).unwrap();
        fs::create_dir_all(root.join("instruments")).unwrap();
        fs::create_dir_all(root.join("sports")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();

        let recent = vec![
            "sports".to_string(),
            "fruits".to_string(),
            "missing".to_string(),
        ];
        assert_eq!(
            get_subdirs(root, &recent),
            vec!["sports", "fruits", "documents", "instruments"]
        );
    }

    #[test]
    fn resolve_collision_adds_incrementing_suffix_before_extension() {
        let tmp = TestDir::new("resolve-collision");
        let root = tmp.path();

        write_file(&root.join("report.txt"));
        write_file(&root.join("report_1.txt"));

        assert_eq!(
            resolve_collision(&root.join("report.txt")),
            root.join("report_2.txt")
        );
    }
}
