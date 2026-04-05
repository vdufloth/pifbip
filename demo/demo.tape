Output demo/demo.gif
Set Width 1000
Set Height 600
Set FontSize 14
Set Theme "Dracula"

# Restore demo files
Type "./demo/restore.sh"
Enter
Sleep 3s

# Show source files
Type "ls -lR demo/source/"
Enter
Sleep 2s

# Show destination (empty)
Type "ls -lR demo/destination/"
Enter
Sleep 2s

# Run pifbip
Type "pifbip demo/source/ demo/destination/ -d 2"
Enter
Sleep 2s

# File 1: apples.jpg -> instruments
Type "instruments"
Enter
Sleep 1.5s

# File 2: bananas.jpg -> instruments (autocomplete)
Type "instrumen"
Sleep 1s
Tab
Sleep 0.5s
Enter
Sleep 1.5s

# File 3: guitar.jpg -> sports
Type "sports"
Enter
Sleep 1.5s

# File 4: piano.jpg -> fruits
Type "fruits"
Enter
Sleep 1.5s

# File 5: soccer.jpg -> fruits
Type "fruits"
Enter
Sleep 1.5s

# File 6: tenis.jpg -> sports
Type "sports"
Enter
Sleep 1.5s

# File 7: text.txt -> skip
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
