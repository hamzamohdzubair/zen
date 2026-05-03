Output assets/board.gif

Set Theme "Dracula"
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 80ms

Hide
Type "export XDG_DATA_HOME=$(mktemp -d) && mkdir -p $XDG_DATA_HOME/zen && cp /home/hmz/src/rs/zen/assets/demo_board.json $XDG_DATA_HOME/zen/tasks.json"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 100ms
Show

Type "zen tui"
Enter
Sleep 900ms

# Switch from Tree mode to Board mode
Tab
Sleep 800ms

# Navigate down to second Todo task (Provision staging server)
Type "j"
Sleep 500ms

# Move it to Doing
Type "L"
Sleep 700ms

# Focus Doing column
Type "l"
Sleep 500ms

# Navigate down one
Type "j"
Sleep 500ms

# Move to Done
Type "L"
Sleep 700ms

# Focus Done column
Type "l"
Sleep 2500ms

Type "q"
