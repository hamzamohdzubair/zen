Output assets/edit-undo.gif

Set Theme "Dracula"
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 80ms

Hide
Type "export XDG_DATA_HOME=$(mktemp -d) && mkdir -p $XDG_DATA_HOME/zen && cp /home/hmz/src/rs/zen/assets/demo_tree.json $XDG_DATA_HOME/zen/tasks.json"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 100ms
Show

Type "zen tui"
Enter
Sleep 1200ms

# Navigate to Login endpoint
Type "j"
Sleep 900ms

# Edit in append mode — cursor lands at end of title
Type "a"
Sleep 700ms

# Rename it
Type " (deprecated)"
Sleep 1000ms
Enter
Sleep 1500ms

# Undo — watch the original title come back
Type "u"
Sleep 3500ms

Type "q"
