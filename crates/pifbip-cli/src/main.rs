mod preview;
mod prompt;
mod viewer;

use std::path::PathBuf;
use std::process;

use clap::{Parser, ValueEnum};
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};

use pifbip_core::{format_size, has_ffmpeg, has_pdftoppm, Outcome, SortSession};
use preview::{has_chafa, show_preview, ImageMode};
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
#[command(name = "pifbip-cli")]
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
            if has_chafa() {
                ImageMode::Chafa
            } else {
                eprintln!("Note: chafa not found, using built-in viewer (install chafa for higher quality)");
                ImageMode::Viuer
            }
        }
    };

    // Create preview window if windowed mode
    let viewer = if matches!(image_mode, ImageMode::Windowed) {
        if !has_ffmpeg() {
            eprintln!("Note: ffmpeg not found, video playback won't work");
        }
        if !has_pdftoppm() {
            eprintln!("Note: pdftoppm not found, PDF preview won't work (install poppler-utils)");
        }
        Some(PreviewWindow::new())
    } else {
        None
    };

    let mut session = SortSession::new(&origin, destination, args.depth);
    let total = session.total();

    if total == 0 {
        println!("No files to sort.");
        return;
    }

    println!(
        "{} file{} to sort. Press Ctrl+C to quit.\n",
        total,
        if total == 1 { "" } else { "s" }
    );

    let mut stdout = std::io::stdout();

    while !session.is_done() {
        let Some(current_path) = session.current() else {
            break;
        };

        // Skip if file no longer exists (shouldn't happen, but safety)
        if !current_path.exists() {
            session.skip_missing();
            continue;
        }

        // Clear screen
        let _ = execute!(stdout, Clear(ClearType::All), MoveTo(0, 0));

        // Header — always show original filename
        let p = session.progress();
        let original_name = session.current_original_name().unwrap_or_default();
        let size = current_path.metadata().map(|m| m.len()).unwrap_or(0);
        println!(
            "\x1b[1m[{}/{}] {}\x1b[0m  ({}/{})",
            p.index + 1,
            p.total,
            original_name,
            p.moved + p.skipped,
            p.total
        );
        println!("Size: {}", format_size(size));
        println!();

        // Preview
        show_preview(&current_path, &image_mode, viewer.as_ref());
        println!();

        // Hint and prompt
        println!("\x1b[2mMoved: {}  Skipped: {}  |  Tab=accept  Up/Down=navigate  Left=undo  Right=skip  Ctrl+R=rename  Esc=quit\x1b[0m", p.moved, p.skipped);
        let existing_dirs = session.subdir_listing();
        let dest = session.destination().to_path_buf();
        match ask_destination(&existing_dirs, &dest) {
            PromptResult::Input(subfolder) => match session.move_to(&subfolder) {
                Outcome::Moved { subfolder, dest_name } => {
                    println!("  Moved -> {}/{}", subfolder, dest_name);
                }
                Outcome::MoveError(e) => eprintln!("  Error {}", e),
                _ => {}
            },
            PromptResult::Skip => match session.skip() {
                Outcome::MoveError(e) => eprintln!("  Error {}", e),
                _ => println!("  Skipped."),
            },
            PromptResult::GoBack => match session.go_back() {
                Outcome::AtStart => println!("  Already at the first file."),
                Outcome::MoveError(e) => eprintln!("  Error {}", e),
                _ => {}
            },
            PromptResult::Interrupted => break,
        }
    }

    // viewer is dropped here, closing the preview window
    drop(viewer);

    let p = session.progress();
    println!(
        "\nDone. Moved: {}, Skipped: {}, Total: {}",
        p.moved, p.skipped, p.total
    );
}
