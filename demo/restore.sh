#!/bin/bash
# Moves all files from destination subfolders back to source, then cleans destination.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE="$SCRIPT_DIR/source"
SUBFOLDER="$SCRIPT_DIR/source/subfoldersource"
DESTINATION="$SCRIPT_DIR/destination"

mkdir -p "$SUBFOLDER"

# Collect all files from destination
files=()
while IFS= read -r -d '' f; do
    files+=("$f")
done < <(find "$DESTINATION" -type f -not -name ".gitkeep" -print0 | sort -z)

# First 2 files go to subfoldersource, rest go to source
count=0
for f in "${files[@]}"; do
    if [ "$count" -lt 2 ]; then
        mv -n "$f" "$SUBFOLDER/"
    else
        mv -n "$f" "$SOURCE/"
    fi
    count=$((count + 1))
done

# Remove empty subdirectories in destination
find "$DESTINATION" -mindepth 1 -type d -delete

echo "Restored. source/ has $(find "$SOURCE" -maxdepth 1 -type f | wc -l) files, subfoldersource/ has $(ls -1 "$SUBFOLDER" 2>/dev/null | wc -l) files."
