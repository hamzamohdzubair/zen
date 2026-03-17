# Zen

A spaced repetition CLI for active recall using the FSRS algorithm.

## Features

- **Simple card creation**: Create flashcards directly from the command line
- **Question-answer format**: Focus on what matters - questions and answers
- **FSRS algorithm**: Uses the modern Free Spaced Repetition Scheduler for optimal learning
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

You'll be prompted to enter the answer (multi-line supported). Press Ctrl+D or enter an empty line to finish.

### Future commands (coming soon)

```bash
zen find google cloud    # Fuzzy search for cards
zen f google cloud       # Short alias
zen start                # Start review session
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
- [ ] **Phase 2**: Fuzzy search and editing with TUI
- [ ] **Phase 3**: Review sessions with FSRS scheduling
- [ ] **Phase 4**: Statistics and polish
- [ ] **Phase 5**: Import/export and advanced features

See [DESIGN.md](DESIGN.md) for detailed feature planning.

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
