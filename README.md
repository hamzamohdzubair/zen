# Zen - Topic-Based Spaced Repetition CLI

A modern spaced repetition CLI that uses LLM-powered question generation and evaluation to help you learn any topic effectively.

## Features

- **Topic-Based Learning**: Organize knowledge by keywords instead of individual flashcards
- **LLM-Powered Questions**: Fresh questions generated for each review session
- **Automatic Grading**: LLM evaluates your answers and provides detailed feedback
- **FSRS Scheduling**: Advanced spaced repetition algorithm for optimal review timing
- **3-Question Review**: Each topic tested with 3 different questions per session
- **Automatic Rating**: Your scores (0-100) automatically convert to SRS ratings
- **Beautiful TUI**: Clean, intuitive terminal interface

## Installation

```bash
cargo install --path .
```

## Quick Start

### 1. Configure LLM

Create `~/.zen/config.toml`:

```toml
[llm]
provider = "groq"
api_key = "your-groq-api-key"
model = "llama-3.3-70b-versatile"
```

Get a free Groq API key at [groq.com](https://groq.com)

### 2. Add Topics

```bash
# Single keyword
zen add "LSTM"

# Multiple related keywords
zen add "Google Cloud, GCP, cloud services"
zen add "AI, RMSE, metrics, evaluation, model assessment"
```

**Note**: Comma separates keywords, spaces are part of each keyword.

### 3. Review Topics

```bash
zen start
```

The review process:
1. See your topic keywords
2. LLM generates a question
3. Type your answer (multi-line supported)
4. LLM grades your answer (0-100) and provides feedback
5. Repeat for 3 questions total
6. Average score determines next review date

### 4. Track Progress

```bash
# View all topics
zen topics

# View only due topics
zen topics --due

# See detailed statistics (TUI)
zen stats
```

The `stats` command opens an interactive TUI with two screens:
- **Topic Performance**: Shows each topic with keywords, last/average scores, and question-wise performance matrix
- **Keyword Performance**: Shows aggregated statistics for each keyword with performance across all topics

Navigation:
- `Tab` - Switch between Topic and Keyword views
- `↑/↓` or `j/l` - Scroll up/down
- `PgUp/PgDn` - Scroll by 10 items
- `Home/End` - Jump to top/bottom
- `q` - Quit

### 5. Manage Topics

```bash
# Delete a topic
zen delete <topic-id>
```

## How It Works

### Topic Reviews

Each review session:
- LLM generates **3 unique questions** covering your topic
- You answer each question in the TUI
- LLM evaluates each answer (0-100 score + feedback)
- Average score converts to FSRS rating:
  - **90%+ → Easy** (long interval)
  - **70-89% → Good** (medium interval)
  - **60-69% → Hard** (short interval)
  - **<60% → Again** (very short interval)

### Score-to-Rating Conversion

The app automatically determines your rating based on LLM scores:

| Score Range | Rating | Next Review |
|-------------|--------|-------------|
| 90-100%     | Easy   | Weeks/months later |
| 70-89%      | Good   | Days/weeks later |
| 60-69%      | Hard   | Days later |
| 0-59%       | Again  | Hours/1 day later |

### FSRS Algorithm

Uses the Free Spaced Repetition Scheduler (FSRS) algorithm for optimal review timing:
- **Stability**: How well you remember
- **Difficulty**: How hard the topic is for you
- **Retrievability**: Probability of recall

The algorithm adapts to your performance and schedules reviews at the optimal time for long-term retention.

## Commands

```bash
zen add <keywords>      # Add a new topic
zen start               # Start review session
zen topics              # List all topics
zen topics --due        # List only due topics
zen stats               # Show detailed statistics (TUI)
zen delete <id>         # Delete a topic
zen --help              # Show help
zen --version           # Show version
```

## Examples

### Adding Topics

```bash
# Machine Learning concepts
zen add "neural networks, backpropagation, gradient descent"

# Programming languages
zen add "Rust, ownership, borrowing"

# Math concepts
zen add "calculus, derivatives, chain rule"

# Business concepts
zen add "product-market fit, MVP, lean startup"
```

### Review Session Example

```
┌─ Topic 1 of 3 | ID: 7jlHGY ──────────────────┐
│ LSTM, recurrent neural networks, time series │
└───────────────────────────────────────────────┘

┌─ Question 1 of 3 ─────────────────────────────┐
│ How does an LSTM differ from a standard RNN   │
│ in terms of handling the vanishing gradient   │
│ problem?                                       │
└───────────────────────────────────────────────┘

┌─ Your Answer (Press Space to start typing) ───┐
│                                                │
└────────────────────────────────────────────────┘
```

After answering, you'll see:

```
┌─ Score: 85/100 (Good) ────────────────────────┐
│ Good explanation of gates and memory cells.   │
│ Could have mentioned the forget gate's role.  │
└────────────────────────────────────────────────┘
```

## Performance Statistics

The `zen stats` command provides detailed statistics in an interactive TUI with two screens:

### Topic Performance Screen

Shows all topics sorted by performance (lowest scores first) with a statistics table on the right:

```
                    Topic Performance

Keywords                Last  Avg  Recent Sessions  ┃              Topic Keyword
────────────────────────────────────────────────────┃ Total           15     42
LSTM, RNN              65.0 72.5  · · · · · · ✗ ✓ - ┃ Due Today        3      5
                                  · · · · · · ✓ - ✗ ┃ Due Week         8     12
                                  · · · · · · - ✗ ✓ ┃ ─────────────────────────
                                                     ┃ Reviews         95
rust, ownership        78.3 80.1  · · · · · ✓ - ✓ - ┃ Avg Score   75.2%  74.8%
                                  · · · · · - ✓ ✗ ✓ ┗━━━━━━━━━━━━━━━━━━━━━━━━
                                  · · · · · ✓ - - -
```

**Layout:**
- **Left**: Topic list with performance matrices
- **Right**: Statistics table comparing Topic and Keyword metrics

**Fields:**
- **Keywords**: Topic keywords (no IDs shown)
- **Last**: Score from most recent review session
- **Avg**: Overall average score across all reviews
- **Recent Sessions**: Fixed 10-column × 3-row grid
  - Each column = one review session with 3 questions
  - Rightmost column = most recent session
  - Symbols: `✓` Easy (≥90), `-` Good/Hard (60-89), `✗` Again (<60), `·` No data

### Keyword Performance Screen

Shows aggregated statistics for each keyword with performance across topics:

```
╔════════════════════════════════════════════════════════════════╗
║ Keyword Performance                                            ║
║ Keywords: 42 | Due Today: 5 | Due Week: 12 | Avg: 74.8%      ║
╚════════════════════════════════════════════════════════════════╝

Keyword                       Topics Avg    Performance by Topic (rightmost = most recent)
──────────────────────────────────────────────────────────────────────────────────────────
LSTM                          3      68.5   · · · · · · · ✗ - ✓
                                            · · · · · · · ✓ ✗ -
                                            · · · · · · · - ✓ ✗
```

**Fields:**
- **Keyword**: The keyword text
- **Topics**: Number of topics containing this keyword
- **Avg**: Average score across all reviews for this keyword
- **Performance Matrix**: Fixed 10-column × 3-row grid showing performance across topics
  - Each column = 3 questions from one topic's most recent session
  - If "LSTM" appears in 3 topics, rightmost 3 columns show those topics
  - Shows how this keyword performs across different contexts
  - Same color coding: Green (≥90), Yellow (60-89), Red (<60), Gray (no data)

**Important**:
- When you switch between views using Tab, the summary statistics update to show metrics relevant to that view
- Keyword "Due Today" and "Due Week" counts show unique keywords in due topics
- Both screens are sorted ascending by average score, so topics/keywords that need more practice appear at the top

## Tips

### Effective Keyword Selection

- **Specific**: "LSTM architecture" is better than just "AI"
- **Related**: Group keywords that belong together
- **Memorable**: Use keywords that trigger the right mental model

### Good Topics vs Bad Topics

✅ **Good**: `"React hooks, useState, useEffect, component lifecycle"`
- Related concepts
- Right level of granularity
- Clear scope

❌ **Bad**: `"programming"`
- Too broad
- No clear scope
- LLM can't generate focused questions

### Review Best Practices

- **Be honest**: Don't look up answers during review
- **Type freely**: Multi-line answers are encouraged
- **Review regularly**: The algorithm works best with consistent reviews
- **Trust the LLM**: The grading is strict but fair

## Configuration

### LLM Providers

Currently supports:
- **Groq** (recommended - fast and free tier available)

Configuration file: `~/.zen/config.toml`

```toml
[llm]
provider = "groq"
api_key = "your-api-key"
model = "llama-3.3-70b-versatile"
```

### Data Location

All data stored in `~/.zen/`:
- `zen.db` - SQLite database with topics, schedules, and review history
- `config.toml` - Configuration file

## Architecture

### Database Schema

```
topics
├── id (TEXT)
├── created_at (TIMESTAMP)
└── modified_at (TIMESTAMP)

topic_keywords
├── topic_id (FK)
├── keyword (TEXT)
└── position (INTEGER)

topic_schedule
├── topic_id (FK)
├── due_date (TIMESTAMP)
├── stability (REAL)
├── difficulty (REAL)
└── retrievability (REAL)

topic_review_logs
├── topic_id (FK)
├── timestamp (TIMESTAMP)
├── rating (1-4)
└── average_score (0-100)

topic_question_logs
├── review_log_id (FK)
├── question_number (1-3)
├── generated_question (TEXT)
├── user_answer (TEXT)
├── llm_score (0-100)
└── llm_feedback (TEXT)
```

## Development

### Building

```bash
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Module exports
├── commands.rs          # Command implementations
├── database.rs          # SQLite operations
├── topic.rs             # Topic data structures
├── topic_review.rs      # Review session logic
├── topic_review_tui.rs  # TUI application
├── llm_evaluator.rs     # LLM integration
└── config.rs            # Configuration management
```

## License

MIT

## Contributing

Contributions welcome! Please open an issue or PR.

## Roadmap

- [ ] Add more LLM providers (OpenAI, Anthropic, local models)
- [ ] Export/import topics
- [ ] Study streak tracking
- [ ] Topic categories/tags
- [ ] Mobile app (see ANDROID_GUIDE.md)
- [ ] Web interface
