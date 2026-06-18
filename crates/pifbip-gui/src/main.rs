mod app;
mod theme;
mod video;

use std::path::PathBuf;

use clap::Parser;

use app::{App, Flags};

#[derive(Parser)]
#[command(name = "pifbip-gui")]
#[command(about = "Put In Folder By Interactive Prompt — desktop GUI file sorter")]
struct Args {
    /// Source folder with files to sort (optional; pick in the GUI otherwise)
    origin: Option<PathBuf>,

    /// Destination folder for sorted files (optional; pick in the GUI otherwise)
    destination: Option<PathBuf>,

    /// How deep to scan source subfolders for files (0=top level only)
    #[arg(short, long, default_value_t = 0)]
    depth: u16,
}

fn main() -> iced::Result {
    let args = Args::parse();
    let flags = Flags {
        origin: args.origin,
        destination: args.destination,
        depth: args.depth,
    };

    iced::application("pifbip — file sorter", App::update, App::view)
        .theme(|_state| theme::theme())
        .subscription(App::subscription)
        .run_with(move || App::new(flags.clone()))
}
