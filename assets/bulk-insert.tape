Output assets/bulk-insert.gif

Set Theme "Dracula"
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 80ms

Hide
Type "export XDG_DATA_HOME=$(mktemp -d) && mkdir -p $XDG_DATA_HOME/zen && cp /home/hmz/src/rs/zen/assets/demo_bulk.json $XDG_DATA_HOME/zen/tasks.json"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 100ms
Show

Type "zen tui"
Enter
Sleep 1200ms

# Cursor is on Sprint 12 — bulk-insert children under it
Type "A"
Sleep 800ms

# Enter the number of tasks to create
Type "4"
Sleep 700ms
Enter
Sleep 800ms

# Enter the task title prefix
Type "Task"
Sleep 700ms
Enter
Sleep 2000ms

# Navigate through the new subtasks
Type "j"
Sleep 600ms
Type "j"
Sleep 600ms
Type "j"
Sleep 600ms
Type "j"
Sleep 3500ms

Type "q"
