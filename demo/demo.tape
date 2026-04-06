Output demo/demo.gif
Set Width 1000
Set Height 600
Set FontSize 14

# Build
Type "cargo build --release 2>&1 | tail -1"
Enter
Sleep 2s

# Restore demo files
Type "./demo/restore.sh"
Enter
Sleep 4s

# Show source files
Type "ls -lR demo/source/"
Enter
Sleep 3s

# Show destination (empty)
Type "ls -lR demo/destination/"
Enter
Sleep 5s

# Run pifbip
Type "./target/release/pifbip demo/source/ demo/destination/ -d 2"
Enter
Sleep 3s

# File 1: guitar.jpg -> instruments
Type "instruments"
Enter
Sleep 1.5s

# File 2: piano.jpg -> instruments (autocomplete)
Type "instrumen"
Sleep 1s
Tab
Sleep 0.5s
Enter
Sleep 1.5s

# File 3: sample-pdf.pdf -> documents
Type "documents"
Enter
Sleep 1.5s

# File 4: soccer.jpg -> sports
Type "sports"
Enter
Sleep 1.5s

# File 5: apples.jpg -> fruits
Type "fruits"
Enter
Sleep 1.5s

# File 6: bananas.jpg -> fruits
Type "fruits"
Enter
Sleep 1.5s

# File 7: tenis.jpg -> sports
Type "sports"
Enter
Sleep 1.5s

# File 8: text.txt -> skip
Enter
Sleep 2s

# Show source after (should have text.txt left)
Type "ls -lR demo/source/"
Enter
Sleep 2s

# Show destination after
Type "ls -lR demo/destination/"
Enter
Sleep 3s
