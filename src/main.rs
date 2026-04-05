mod files;
mod preview;
mod prompt;
mod viewer;

use std::path::PathBuf;
use std::process;

use clap::{Parser, ValueEnum};
use crossterm::{execute, terminal::{Clear, ClearType}, cursor::MoveTo};

use files::{collect_files, format_size, get_subdirs, move_file, resolve_collision};
use preview::{show_preview, ImageMode};
use prompt::{ask_destination, PromptResult};
use viewer::PreviewWindow;

#[derive(Clone, ValueEnum)]
enum ImageModeArg {
    Auto,
    Chafa,
    Viuer,
    Windowed,
}

#[derive(Parser)]
#[command(name = "pifbip")]
#[command(about = "Put In Folder By Interactive Prompt — fast manual file sorting")]
struct Args {
    /// Source folder with files to sort
    origin: PathBuf,

    /// Destination folder for sorted files
    destination: PathBuf,

    /// How deep to scan source subfolders for files (0=top level only)
    #[arg(short, long, default_value_t = 0)]
    depth: u16,

    /// Image preview mode: auto (chafa if available, else viuer), chafa, viuer, or windowed (GUI preview window)
    #[arg(long, value_enum, default_value_t = ImageModeArg::Auto)]
    image_mode: ImageModeArg,
}

fn main() {
    // Ensure terminal is restored on panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        default_hook(info);
    }));

    let args = Args::parse();

    let origin = match args.origin.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: origin '{}': {}", args.origin.display(), e);
            process::exit(1);
        }
    };
    let destination = match args.destination.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: destination '{}': {}", args.destination.display(), e);
            process::exit(1);
        }
    };

    if !origin.is_dir() {
        eprintln!("Error: origin '{}' is not a directory", origin.display());
        process::exit(1);
    }
    if !destination.is_dir() {
        eprintln!("Error: destination '{}' is not a directory", destination.display());
        process::exit(1);
    }

    let image_mode = match args.image_mode {
        ImageModeArg::Chafa => ImageMode::Chafa,
        ImageModeArg::Viuer => ImageMode::Viuer,
        ImageModeArg::Windowed => ImageMode::Windowed,
        ImageModeArg::Auto => {
            if preview::has_chafa() {
                ImageMode::Chafa
            } else {
                eprintln!("Note: chafa not found, using built-in viewer (install chafa for higher quality)");
                ImageMode::Viuer
            }
        }
    };

    // Create preview window if windowed mode
    let viewer = if matches!(image_mode, ImageMode::Windowed) {
        if !preview::has_ffmpeg() {
            eprintln!("Note: ffmpeg not found, video playback in windowed mode won't work");
        }
        Some(PreviewWindow::new())
    } else {
        None
    };

    let file_list = collect_files(&origin, args.depth);
    let total = file_list.len();

    if total == 0 {
        println!("No files to sort.");
        return;
    }

    println!(
        "{} file{} to sort. Press Ctrl+C to quit.\n",
        total,
        if total == 1 { "" } else { "s" }
    );

    let mut moved = 0usize;
    let mut stdout = std::io::stdout();
    let mut recent_dirs: Vec<String> = Vec::new();

    // History for undo: tracks what happened to each file
    // None = skipped or not yet visited, Some(dest_path) = moved to this path
    let mut history: Vec<Option<PathBuf>> = vec![None; total];
    let mut i = 0usize;

    while i < total {
        let filepath = &file_list[i];

        // If the file was already moved (we came back via undo), it's at the history path
        let current_path = if let Some(ref moved_to) = history[i] {
            moved_to.clone()
        } else {
            filepath.clone()
        };

        // Skip if file no longer exists (shouldn't happen, but safety)
        if !current_path.exists() {
            i += 1;
            continue;
        }

        // Clear screen
        let _ = execute!(stdout, Clear(ClearType::All), MoveTo(0, 0));

        // Header — always show original filename
        let original_name = filepath.file_name().unwrap_or_default().to_string_lossy();
        let size = current_path.metadata().map(|m| m.len()).unwrap_or(0);
        println!("\x1b[1m[{}/{}] {}\x1b[0m", i + 1, total, original_name);
        println!("Size: {}", format_size(size));
        println!();

        // Preview
        show_preview(&current_path, &image_mode, viewer.as_ref());
        println!();

        // Hint and prompt
        println!("\x1b[2mMoved: {}/{}  |  Tab=accept  Up/Down=navigate  Left=undo  Right=skip  Ctrl+R=rename  Esc=quit\x1b[0m", moved, total);
        let existing_dirs = get_subdirs(&destination, &recent_dirs);
        match ask_destination(&existing_dirs, &destination) {
            PromptResult::Input(subfolder) => {
                // If file was previously moved (undo case), move it back from its current location
                let source = &current_path;
                let target_dir = destination.join(&subfolder);
                if let Err(e) = std::fs::create_dir_all(&target_dir) {
                    eprintln!("  Error creating directory: {}", e);
                    continue;
                }
                let original_name = file_list[i].file_name().unwrap_or_default().to_string_lossy().to_string();
                let dest_file = resolve_collision(&target_dir.join(&original_name));
                match move_file(source, &dest_file) {
                    Ok(()) => {
                        // If this was a re-do of a previously moved file, adjust count
                        if history[i].is_some() {
                            // Already counted, don't increment
                        } else {
                            moved += 1;
                        }
                        history[i] = Some(dest_file.clone());
                        recent_dirs.retain(|d| d != &subfolder);
                        recent_dirs.insert(0, subfolder.clone());
                        let dest_name = dest_file.file_name().unwrap_or_default().to_string_lossy();
                        println!("  Moved -> {}/{}", subfolder, dest_name);
                    }
                    Err(e) => eprintln!("  Error moving file: {}", e),
                }
                i += 1;
            }
            PromptResult::Skip => {
                // If file was previously moved, move it back to origin
                if let Some(ref moved_path) = history[i] {
                    let original = &file_list[i];
                    if let Err(e) = move_file(moved_path, original) {
                        eprintln!("  Error undoing move: {}", e);
                    } else {
                        moved -= 1;
                        println!("  Undone, file restored to source.");
                    }
                    history[i] = None;
                } else {
                    println!("  Skipped.");
                }
                i += 1;
            }
            PromptResult::GoBack => {
                if i > 0 {
                    // Go back: undo the previous file's move if it was moved
                    i -= 1;
                    if let Some(ref moved_path) = history[i] {
                        let original = &file_list[i];
                        if let Err(e) = move_file(moved_path, original) {
                            eprintln!("  Error undoing move: {}", e);
                        } else {
                            moved -= 1;
                            history[i] = None;
                        }
                    }
                } else {
                    println!("  Already at the first file.");
                }
            }
            PromptResult::Interrupted => {
                break;
            }
        }
    }

    // viewer is dropped here, closing the preview window
    drop(viewer);

    println!("\nDone. Moved {}/{} files.", moved, total);
}
