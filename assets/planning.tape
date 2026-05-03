Output assets/planning.gif

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

# Navigate down to Dashboard
Type "j"
Sleep 700ms
Type "j"
Sleep 700ms
Type "j"
Sleep 1000ms

# Insert a new sibling after Dashboard
Type "o"
Sleep 500ms
Type "Logout flow"
Sleep 900ms
Enter
Sleep 1200ms

# Indent it — becomes a child of Dashboard
Type ">"
Sleep 1800ms

# Reorder it upward among Dashboard's children
Type "K"
Sleep 1200ms

# Navigate up to Dashboard and fold the branch
Type "k"
Sleep 700ms
Type "k"
Sleep 700ms
Type "h"
Sleep 1500ms

# Unfold to reveal the structure again
Type "l"
Sleep 3500ms

Type "q"
