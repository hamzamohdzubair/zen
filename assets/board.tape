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
Sleep 1200ms

# Switch from Tree to Board mode
Tab
Sleep 1500ms

# Navigate down to the second Todo task (Provision staging server)
Type "j"
Sleep 900ms

# Move it to Doing — watch the columns update
Type "L"
Sleep 1800ms

# Focus the Doing column
Type "l"
Sleep 900ms

# Navigate down to the second Doing task
Type "j"
Sleep 800ms

# Move it to Done
Type "L"
Sleep 1800ms

# Focus Done to see the completed task land there
Type "l"
Sleep 3500ms

Type "q"
