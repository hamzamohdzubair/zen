# zen

A modal, keyboard-driven kanban TUI for personal task management.

Tasks live in a tree. You plan structure in **Tree mode** and execute work in **Board mode**. Both views share the same data and stay in sync.

---

## Install

```bash
cargo install --path .
```

## Usage

```bash
zen           # launch the TUI (default)
zen tui       # same as above
zen done      # browse completed tasks
zen stats     # project statistics
zen export    # export all tasks as CSV
zen export done  # export completed tasks with timing metadata
```

---

## Views

### Board mode — ACTION

Three-column kanban: **Todo · Doing · Done**

- Only leaf tasks are shown (tasks with no active children)
- **Todo** sorted by project age or slot priority (toggle with `s`)
- **Doing** sorted by tree order; reorderable with `J`/`K`
- **Done** sorted by completion date, newest first
- A project summary table above the board shows remaining tasks, completion %, and a progress bar per project

### Tree mode — PLAN

Full task hierarchy for a project.

- Tree connectors (├─ ╰─) show depth and structure
- Navigate the full tree regardless of task status
- Only leaf tasks with no active children appear on the board
- Parent status auto-derives from children: if all children are Done, the parent becomes Done and that propagates to the root

---

## Keybindings

### Board mode

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `h` / `l` | Focus previous / next column |
| `H` / `L` | Move selected task left / right (change status) |
| `J` / `K` | Reorder within Doing column |
| `s` | Cycle Todo sort: Age ↔ Project |
| `Enter` | Jump to Tree mode for the selected task |
| `Tab` | Jump to Tree mode for the highest-priority project |

### Tree mode

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `,` / `.` | Cycle to previous / next project |
| `Enter` | Return to Board, cursor on selected task |
| `Tab` / `Backspace` | Return to Board |
| `o` / `O` | Insert sibling after / before |
| `i` / `a` | Edit title (cursor at start / end) |
| `d` `d` | Delete selected task (double-press within 500 ms) |
| `u` | Undo (50-step stack) |
| `>` | Indent — make child of task above (one level at a time) |
| `<` | Outdent — promote one level toward root |
| `J` / `K` | Move task down / up within its siblings |
| `m` | Assign project (Move mode) |
| `A` | Bulk insert — create N children with a prefix |
| `h` / `l` | Fold / toggle-fold selected branch |

### Insert mode

| Key | Action |
|-----|--------|
| `Enter` | Confirm |
| `Esc` | Cancel |
| `Tab` / `Shift+Tab` | Indent / unindent the new task |

### Global

| Key | Action |
|-----|--------|
| `1`–`9`, `0` | Toggle project slot visibility (Tree) / open project in Tree (Board) |
| `` ` `` | Toggle INBOX visibility / open INBOX in Tree |
| `=` / `-` | Enable all / disable all projects |
| `P` | Edit project slot names |
| `?` | Toggle help overlay |
| `q` | Quit |

---

## Projects

Ten named project slots mapped to keys `1`–`9` and `0`. An eleventh implicit slot — **INBOX** (`` ` ``) — holds tasks with no assigned project.

- Assign a project to a task with `m` in Tree mode
- When a task is made a child (`>`), it inherits the parent's project; if the parent has none, the whole family adopts the child's project
- Toggle which projects are visible with their slot key; `=` shows all, `-` hides all

---

## Data

Tasks are saved automatically to `~/.local/share/zen/tasks.json` on every modification. No explicit save step.

```
~/.local/share/zen/tasks.json
```

The file stores all tasks and the project slot names. It is human-readable JSON and safe to back up or version-control.

---

## Export

```bash
zen export       # all tasks: id, title, project, status, created_at, time in each status (seconds)
zen export done  # completed tasks: above + completed_at, total elapsed, time in todo/doing
```
