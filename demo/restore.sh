#!/bin/bash
# Restores all demo files to their original locations.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE="$SCRIPT_DIR/source"
SUBFOLDER="$SCRIPT_DIR/source/subfoldersource"
DESTINATION="$SCRIPT_DIR/destination"

mkdir -p "$SOURCE" "$SUBFOLDER"

# Move all files from destination back to source (flat)
while IFS= read -r -d '' f; do
    mv -n "$f" "$SOURCE/"
done < <(find "$DESTINATION" -type f -not -name ".gitkeep" -print0)

# Remove empty subdirectories in destination
find "$DESTINATION" -mindepth 1 -type d -delete 2>/dev/null

# Now place the specific files that belong in subfoldersource
for name in apples.jpg bananas.jpg; do
    if [ -f "$SOURCE/$name" ]; then
        mv -n "$SOURCE/$name" "$SUBFOLDER/"
    fi
done

echo "Restored. source/ has $(find "$SOURCE" -maxdepth 1 -type f | wc -l) files, subfoldersource/ has $(ls -1 "$SUBFOLDER" 2>/dev/null | wc -l) files."
