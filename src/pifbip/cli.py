import argparse
import shutil
import sys
from pathlib import Path

from pifbip.preview import show_preview
from pifbip.prompt import ask_destination


def get_files(origin: Path, depth: int = 0) -> list[Path]:
    """Get sorted list of non-hidden files in origin, scanning up to depth levels deep."""
    files = []
    _collect_files(origin, depth, 0, files)
    return sorted(files)


def _collect_files(directory: Path, max_depth: int, current_depth: int, result: list[Path]) -> None:
    for entry in directory.iterdir():
        if entry.name.startswith("."):
            continue
        if entry.is_file():
            result.append(entry)
        elif entry.is_dir() and current_depth < max_depth:
            _collect_files(entry, max_depth, current_depth + 1, result)


def get_subdirs(destination: Path) -> list[str]:
    """Get list of existing subdirectory names in destination."""
    return sorted(
        d.name for d in destination.iterdir()
        if d.is_dir()
    )


def resolve_collision(dest_file: Path) -> Path:
    """If dest_file exists, append _1, _2, etc. before the extension."""
    if not dest_file.exists():
        return dest_file
    stem = dest_file.stem
    suffix = dest_file.suffix
    parent = dest_file.parent
    counter = 1
    while True:
        candidate = parent / f"{stem}_{counter}{suffix}"
        if not candidate.exists():
            return candidate
        counter += 1


def main():
    parser = argparse.ArgumentParser(
        prog="pifbip",
        description="Put In Folder By Interactive Prompt — fast manual file sorting",
    )
    parser.add_argument("origin", type=Path, help="Source folder with files to sort")
    parser.add_argument("destination", type=Path, help="Destination folder for sorted files")
    parser.add_argument(
        "-d", "--depth", type=int, default=0,
        help="How deep to scan source subfolders for files (0=top level only, 1=one level deep, etc.)",
    )
    args = parser.parse_args()

    origin: Path = args.origin.resolve()
    destination: Path = args.destination.resolve()

    if not origin.is_dir():
        print(f"Error: origin '{origin}' is not a directory", file=sys.stderr)
        sys.exit(1)
    if not destination.is_dir():
        print(f"Error: destination '{destination}' is not a directory", file=sys.stderr)
        sys.exit(1)

    files = get_files(origin, depth=args.depth)
    total = len(files)
    if total == 0:
        print("No files to sort.")
        return

    print(f"{total} file{'s' if total != 1 else ''} to sort. Press Ctrl+C to quit.\n")

    moved = 0
    try:
        for i, filepath in enumerate(files, 1):
            # Clear screen
            print("\033[2J\033[H", end="")

            # Header
            print(f"\033[1m[{i}/{total}] {filepath.name}\033[0m")
            size = filepath.stat().st_size
            print(f"Size: {_format_size(size)}")
            print()

            # Preview
            show_preview(filepath)
            print()

            # Prompt
            existing_dirs = get_subdirs(destination)
            subfolder = ask_destination(existing_dirs)

            if not subfolder:
                print("  Skipped.")
                continue

            # Move file
            target_dir = destination / subfolder
            target_dir.mkdir(parents=True, exist_ok=True)
            dest_file = resolve_collision(target_dir / filepath.name)
            shutil.move(str(filepath), str(dest_file))
            moved += 1
            print(f"  Moved -> {subfolder}/{dest_file.name}")

    except (KeyboardInterrupt, EOFError):
        print()

    print(f"\nDone. Moved {moved}/{total} files.")


def _format_size(size: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if size < 1024:
            return f"{size:.1f} {unit}" if unit != "B" else f"{size} {unit}"
        size /= 1024
    return f"{size:.1f} TB"


if __name__ == "__main__":
    main()
