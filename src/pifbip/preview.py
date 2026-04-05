import mimetypes
import os
import shutil
import subprocess
from pathlib import Path

IMAGE_EXTENSIONS = {
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".tiff", ".tif",
    ".svg", ".ico",
}

TEXT_EXTENSIONS = {
    ".txt", ".md", ".csv", ".log", ".json", ".xml", ".yaml", ".yml",
    ".html", ".css", ".js", ".py", ".sh", ".conf", ".ini", ".toml",
    ".rs", ".go", ".java", ".c", ".cpp", ".h", ".hpp", ".rb", ".php",
}

HEAD_LINES = 10
TAIL_LINES = 10
MAX_DISPLAY_LINES = HEAD_LINES + TAIL_LINES + 5


def show_preview(filepath: Path) -> None:
    ext = filepath.suffix.lower()
    if ext in IMAGE_EXTENSIONS:
        _preview_image(filepath)
    elif ext in TEXT_EXTENSIONS:
        _preview_text(filepath)
    else:
        _preview_other(filepath)


def _preview_image(filepath: Path) -> None:
    if shutil.which("chafa"):
        try:
            subprocess.run(
                ["chafa", "--size=80x24", "--animate=off", str(filepath)],
                timeout=5,
            )
        except subprocess.TimeoutExpired:
            print("  (preview timed out)")
    else:
        print("  [Install chafa for image preview: sudo apt install chafa]")
        _preview_other(filepath)


def _preview_text(filepath: Path) -> None:
    try:
        cols = os.get_terminal_size().columns
    except OSError:
        cols = 80

    try:
        with open(filepath, errors="replace") as f:
            lines = f.readlines(1024 * 64)  # read up to 64KB
    except OSError as e:
        print(f"  (cannot read file: {e})")
        return

    total = len(lines)
    print(f"  --- {total} line{'s' if total != 1 else ''} ---")

    if total <= MAX_DISPLAY_LINES:
        for line in lines:
            print(f"  {line.rstrip()[:cols - 4]}")
    else:
        for line in lines[:HEAD_LINES]:
            print(f"  {line.rstrip()[:cols - 4]}")
        print(f"  ... ({total - HEAD_LINES - TAIL_LINES} lines omitted) ...")
        for line in lines[-TAIL_LINES:]:
            print(f"  {line.rstrip()[:cols - 4]}")

    print(f"  --- end ---")


def _preview_other(filepath: Path) -> None:
    size = filepath.stat().st_size
    mime, _ = mimetypes.guess_type(str(filepath))
    mime_str = mime or "unknown type"
    print(f"  Type: {mime_str} | Size: {_format_size(size)}")


def _format_size(size: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if size < 1024:
            return f"{size:.1f} {unit}" if unit != "B" else f"{size} {unit}"
        size /= 1024
    return f"{size:.1f} TB"
