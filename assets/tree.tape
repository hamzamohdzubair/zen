Output assets/tree.gif

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
Sleep 900ms

# Navigate down through the tree
Type "j"
Sleep 350ms
Type "j"
Sleep 350ms
Type "j"
Sleep 600ms

# Insert a new sibling after Dashboard
Type "o"
Sleep 300ms
Type "Logout flow"
Sleep 600ms
Enter
Sleep 600ms

# Indent it to become a child of Dashboard
Type ">"
Sleep 1000ms

# Navigate up to Dashboard and fold the branch
Type "k"
Sleep 350ms
Type "k"
Sleep 350ms
Type "h"
Sleep 900ms

# Unfold
Type "l"
Sleep 2000ms

Type "q"
