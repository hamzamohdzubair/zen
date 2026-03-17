# Zen

A spaced repetition CLI for active recall using the FSRS algorithm.

## Features

- **Simple card creation**: Create flashcards directly from the command line
- **Question-answer format**: Focus on what matters - questions and answers
- **FSRS algorithm**: Uses the modern Free Spaced Repetition Scheduler for optimal learning
- **Interactive review sessions**: TUI-based review with real-time interval previews
- **Fuzzy search**: Find and edit cards with interactive fuzzy matching
- **CLI-first**: All interactions through the terminal
- **Easy editing**: Cards stored as simple markdown files
- **Efficient storage**: Hybrid approach with markdown for content and SQLite for scheduling

## Installation

```bash
cargo install --git https://github.com/hamzamohdzubair/zen
```

Or clone and build from source:

```bash
git clone git@github.com:hamzamohdzubair/zen.git
cd zen
cargo build --release
```

## Usage

### Create a new card

```bash
zen new what is the relationship between vpc and vm in google cloud
```

You'll be prompted to enter the answer (multi-line supported). Press Enter twice to finish.

### Find and edit cards

```bash
zen find google cloud    # Fuzzy search for cards
zen f google cloud       # Short alias
```

Opens an interactive fuzzy finder. Type to filter cards, use arrows/Tab to navigate, press Enter to edit in your default editor.

### Start a review session

```bash
zen start
```

Reviews all cards that are due:
- Press `Space` or `Enter` to reveal the answer
- Rate your recall: `1` (Again), `2` (Hard), `3` (Good), `4` (Easy)
- Each rating shows the next review interval (e.g., 10m, 3d, 8d, 21d)
- Progress indicator shows your position (Card 3/10)
- Summary statistics displayed at the end

### Future commands (coming soon)

```bash
zen stats                # Show statistics
zen list                 # List all cards
```

## Card Storage

Cards are stored in `~/.zen/`:

- **Content**: `~/.zen/cards/*.md` - Simple markdown files
- **Metadata**: `~/.zen/zen.db` - SQLite database for scheduling

### Card Format

Cards use a minimal format:

```
What is your question?

---

This is the answer.
```

Everything before `\n\n---\n\n` is the question, everything after is the answer.

## Roadmap

- [x] **Phase 1**: Basic card creation and storage
- [x] **Phase 2**: Fuzzy search and editing with TUI
- [x] **Phase 3**: Review sessions with FSRS scheduling
- [ ] **Phase 4**: Statistics and polish
- [ ] **Phase 5**: Import/export and advanced features

See [DESIGN.md](DESIGN.md) for detailed feature planning.

## How Reviews Work (FSRS)

Zen uses the FSRS (Free Spaced Repetition Scheduler) algorithm to optimize your review schedule:

1. **New cards** start with short intervals (minutes to days)
2. **Each rating** updates the card's memory parameters:
   - **Stability**: How long the memory lasts
   - **Difficulty**: The card's inherent difficulty
   - **Retrievability**: Your current memory strength
3. **Next review date** is calculated to maintain 90% retention probability
4. **Four rating options**:
   - `1 - Again`: Failed recall (review soon, ~10m)
   - `2 - Hard`: Difficult recall (~3d)
   - `3 - Good`: Correct with effort (~8d)
   - `4 - Easy`: Perfect recall (~21d)

Intervals shown are examples for new cards. The algorithm adapts based on your actual performance history.

## Design Principles

- **No full-screen interfaces**: All TUIs are max half-terminal height
- **Vim-like navigation**: Ctrl+j/k for movement
- **No quotes needed**: Natural command syntax
- **Easy editing**: Edit cards in your favorite editor
- **Non-invasive**: Minimal, focused interface

## Development

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Run

```bash
cargo run -- new your question here
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
