# Zen - Spaced Repetition CLI

A command-line spaced repetition and active recall system using the FSRS algorithm.

## Core Concepts

- **FSRS Algorithm**: Free Spaced Repetition Scheduler using Difficulty-Stability-Retrievability model
- **Question-Answer Cards**: Simple flashcard format (only type supported)
- **CLI-First**: All interactions through terminal interface
- **Efficient Storage**: Lightweight storage with easy editing capabilities

## Planned Features

### 1. Card Creation (Phase 1 - MVP)

**Command**: `zen new <question>`

- Create new flashcard with a question
- No quotes required - everything after "new" is the question
- After creating question, prompt for answer input
- Multi-line answer support (Ctrl+D or Ctrl+Enter to finish)
- Auto-generate 6-digit alphanumeric case-sensitive ID for each card

**Storage Options to Research**:
- Option A: Markdown files (one per card, 6-digit filename like `Abc123.md`)
- Option B: SQLite database with text fields
- Option C: Hybrid - markdown files for content, SQLite for scheduling metadata

**Considerations**:
- Easy editing without special tools
- Human-readable format
- Efficient querying for reviews
- Preserve scheduling history separately from content

### 2. Card Storage Format

**Markdown File Format**:
```
<question text>

---

<answer text>
```

- Everything from start until `\n\n---\n\n` is the question
- Everything after the separator is the answer
- Simple, minimal format - no headers or special syntax

**Metadata Storage** (separate from content):
- Card ID
- Creation timestamp
- Last review timestamp
- FSRS memory state (Stability, Difficulty, Retrievability)
- Review history (dates, ratings, intervals)
- Due date for next review

### 3. Fuzzy Search & Editing (Phase 2)

**Command**: `zen find <query>` or `zen f <query>`

**Interface**:
- Half-terminal-height interactive TUI
- Split view:
  - Left pane: List of matching cards (fuzzy search results)
  - Right pane: Preview of selected card content
- Navigation: Ctrl+j (down), Ctrl+k (up)
- Press Enter to edit selected card
- ESC to exit

**Implementation Notes**:
- Use fuzzy matching algorithm (e.g., `skim`, `nucleo`)
- Search both questions and answers
- Real-time preview as you navigate
- Similar UX to `zk` CLI note-taking app

**Editing Behavior**:
- Opens card in $EDITOR (default: vim/nano)
- After editing, detect if content changed
- If changed: Reset scheduling history (treat as new card)
- Preserve card ID but clear FSRS memory state

### 4. Review Session (Phase 3)

**Command**: `zen start`

**Interface Flow**:
1. Calculate which cards are due for review using FSRS
2. Present cards in optimal order
3. For each card:
   - Display question in half-terminal-height view
   - Provide text area for answer input
   - Multi-line input supported
   - Enter key: New line (doesn't submit)
   - Ctrl+Enter: Submit answer and show comparison

**Answer Comparison View**:
- Half-terminal-height split interface
- Left pane: User's answer
- Right pane: Correct answer
- Highlighting:
  - Matching keywords highlighted in both panes
  - Use n-gram matching for partial matches
  - Color-coded: exact matches (green), partial matches (yellow)
- Similarity score at bottom:
  - Calculate using BERT embeddings
  - Display percentage similarity (0-100%)
- Rating prompt: Ask user to rate difficulty (Again, Hard, Good, Easy)
- Update FSRS memory state based on rating

### 5. Statistics & Progress (Phase 4)

**Command**: `zen stats`

- Total cards count
- Cards due today
- Cards due this week
- Average retention rate
- Study streak
- Time spent reviewing

### 6. Card Management (Phase 5)

**Commands**:
- `zen list` - List all cards with basic info
- `zen delete <id>` - Delete a card
- `zen export` - Export cards to Anki format
- `zen import` - Import from other formats

## Technical Architecture

### Dependencies

```toml
# Core FSRS implementation
fsrs = "5.2.0"

# CLI framework
clap = { version = "4", features = ["derive"] }

# TUI framework for interactive interfaces
ratatui = "0.29"
crossterm = "0.28"

# Fuzzy finding
nucleo = "0.5"  # or skim

# Storage
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Text similarity (for answer comparison)
# - For BERT embeddings: rust-bert or candle
# - For simpler similarity: strsim or similar

# Terminal color/styling
colored = "2.1"
```

### Data Models

```rust
// Card content
struct Card {
    id: String,           // 6-digit alphanumeric
    question: String,
    answer: String,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
}

// FSRS scheduling data
struct CardSchedule {
    card_id: String,
    memory_state: Option<MemoryState>,  // None for new cards
    due_date: DateTime<Utc>,
    review_history: Vec<ReviewLog>,
}

// Review log entry
struct ReviewLog {
    timestamp: DateTime<Utc>,
    rating: Rating,  // Again(1), Hard(2), Good(3), Easy(4)
    scheduled_days: f64,
    elapsed_days: f64,
}
```

### Storage Architecture Decision

**Hybrid Approach** (Recommended):
1. **Markdown files** for card content (questions and answers)
   - Easy to edit in any text editor
   - Human-readable
   - Version control friendly
   - Located in `~/.zen/cards/`

2. **SQLite database** for scheduling metadata
   - Efficient querying for due cards
   - Store FSRS memory states
   - Review history
   - Located in `~/.zen/zen.db`

**Benefits**:
- Best of both worlds: easy editing + efficient scheduling
- Clear separation of concerns
- Content is portable (just markdown)
- Scheduling can be rebuilt if needed

### File Structure

```
~/.zen/
├── cards/
│   ├── Abc123.md
│   ├── Def456.md
│   └── ...
└── zen.db (SQLite)
```

## Implementation Phases

### Phase 1: MVP (Current Focus)
- [ ] Basic card creation (`zen new`)
- [ ] Simple storage (markdown + SQLite)
- [ ] ID generation
- [ ] Input handling for questions and answers

### Phase 2: Search & Edit
- [ ] Fuzzy search implementation
- [ ] TUI for search interface
- [ ] Edit detection and schedule reset

### Phase 3: Review Session
- [ ] FSRS integration
- [ ] Due card calculation
- [ ] Review TUI
- [ ] Answer comparison with highlighting
- [ ] BERT embeddings for similarity

### Phase 4: Polish
- [ ] Statistics
- [ ] Better error handling
- [ ] Configuration file
- [ ] Card management commands

### Phase 5: Advanced Features
- [ ] Import/export
- [ ] Card templates
- [ ] Tag system
- [ ] Custom review algorithms

## UI/UX Considerations

1. **Minimal friction**: No quotes needed for commands
2. **Natural input**: Multi-line support everywhere
3. **Vim-like keybindings**: Ctrl+j/k for navigation in TUI
4. **Visual feedback**: Progress bars, colors, clear prompts
5. **Forgiving**: Confirm before destructive actions
6. **Fast**: Instant search results, quick card access
7. **Non-invasive**: Max half-terminal height, never full screen

## References

- FSRS Algorithm: https://github.com/open-spaced-repetition/fsrs-rs
- Similar tools: Anki, zk (note-taking CLI)
- TUI inspiration: `zk`, `lazygit`, `gitui`
