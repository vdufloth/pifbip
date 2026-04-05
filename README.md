# pifbip — Put In Folder By Interactive Prompt

Fast bulk file sorting when there's no pattern and you need to decide manually.

For each file in the source folder, pifbip shows the filename and a preview, then prompts you to type a destination subfolder. As you type, existing folder names appear as fuzzy autocomplete suggestions. The file is moved instantly and the next one loads.

![demo](demo/demo.gif)

## Previews

- **Images** (jpg, png, gif, webp, bmp, etc.) — displayed in the terminal via [chafa](https://hpjansson.org/chafa/) (optional)
- **Text files** (txt, md, csv, json, py, etc.) — first 10 and last 10 lines
- **Other files** — name, size, and MIME type

## Requirements

- Python 3.9+
- [chafa](https://hpjansson.org/chafa/) (optional, for image previews)

## Installation

```bash
pip install .
```

To install chafa for image previews:

```bash
# Fedora/RHEL
sudo dnf install chafa

# Debian/Ubuntu
sudo apt install chafa
```

## Usage

```bash
pifbip <source> <destination> [options]
```

### Options

| Flag | Description |
|---|---|
| `-d`, `--depth N` | How many levels deep to scan source subfolders for files. `0` (default) = only top-level files, `1` = include one level of subfolders, etc. |
| `-h`, `--help` | Show help message and exit |

### Examples

```bash
# Sort files from Downloads into organized folders
pifbip ~/Downloads ~/Sorted

# Include files from subfolders one level deep
pifbip ~/Downloads ~/Sorted -d 1

# Scan all nested subfolders up to 3 levels
pifbip ~/Downloads ~/Sorted -d 3
```

### Controls

- Type a folder name and press **Enter** to move the file there (created if it doesn't exist)
- Press **Enter** on empty input to skip a file
- Press **Ctrl+C** to quit at any time

## Demo

A `demo/` folder is included with sample source files. After sorting, run the restore script to reset:

```bash
./demo/restore.sh
```

To re-record the demo GIF (requires [VHS](https://github.com/charmbracelet/vhs)):

```bash
./demo/restore.sh
vhs demo/demo.tape
```
