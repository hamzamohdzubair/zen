# Zen - AI-Powered Spaced Repetition Learning

**Terminal client for [forgetmeifyoucan.com](https://forgetmeifyoucan.com)** - Never forget what you learn.

Zen is a lightweight CLI/TUI client that connects to your forgetmeifyoucan cloud account, allowing you to review your topics from the comfort of your terminal.

## Features

- 🧠 **AI-powered reviews** - Smart question generation and evaluation
- 📊 **Spaced repetition** - FSRS algorithm for optimal learning
- 💻 **Terminal interface** - Fast, keyboard-driven workflow
- ☁️ **Cloud sync** - Access your topics from anywhere
- 🔐 **Secure** - Token-based authentication with system keyring

## Installation

### From source (Rust required)

```bash
cargo install --git https://github.com/hamzamohdzubair/zen
```

### Pre-built binaries

Download from [Releases](https://github.com/hamzamohdzubair/zen/releases)

## Quick Start

1. **Create an account** at [forgetmeifyoucan.com](https://forgetmeifyoucan.com) (1-month free trial!)

2. **Login from terminal:**
   ```bash
   zen login
   ```

3. **Add topics:**
   ```bash
   zen add "Rust, Programming, Systems"
   zen add "Machine Learning, AI, Neural Networks"
   ```

4. **Start reviewing:**
   ```bash
   zen start
   ```

5. **Check your progress:**
   ```bash
   zen stats
   ```

## Commands

| Command | Description |
|---------|-------------|
| `zen login` | Login to your forgetmeifyoucan account |
| `zen logout` | Logout from your account |
| `zen add "keywords"` | Create a new topic |
| `zen list` | List all your topics |
| `zen list --due` | List only due topics |
| `zen start` | Start a review session |
| `zen stats` | View your learning statistics |
| `zen delete <id>` | Delete a topic |
| `zen me` | Show account information |

## Why Zen?

### For Terminal Enthusiasts
- Work entirely from your terminal
- Fast keyboard-driven interface
- No context switching to browser
- Perfect for SSH/remote sessions

### For Learners
- AI asks you smart questions about your topics
- Spaced repetition ensures you remember
- Track your progress over time
- Never cram again!

## Cloud Version: forgetmeifyoucan

Want the full experience with web and mobile apps?

👉 **Visit [forgetmeifyoucan.com](https://forgetmeifyoucan.com)**

### Features:
- ☁️ Cloud sync across all devices
- 📱 Web app + Mobile apps (coming soon)
- 🤖 Advanced AI-powered reviews
- 📊 Detailed analytics and insights
- 🆓 **1-month free trial**
- 💰 Then $3/month (Basic) or $7/month (Pro)

### Pricing:
- **Trial**: 1 month free (all features)
- **Basic**: $3/month (unlimited topics & reviews)
- **Pro**: $7/month (priority support + advanced features)

## Development

This is a thin client that connects to the forgetmeifyoucan API. All business logic (LLM evaluation, FSRS scheduling, etc.) runs on the cloud.

**Tech stack:**
- Rust
- Clap (CLI parsing)
- Ratatui (TUI)
- Reqwest (API client)
- Keyring (secure token storage)

## License

MIT OR Apache-2.0

## Related Projects

- **[forgetmeifyoucan](https://forgetmeifyoucan.com)** - The cloud platform (private repo)
- **Backend API** - Rust + Axum + PostgreSQL
- **Web frontend** - React + TypeScript
- **Mobile apps** - Coming soon!

## Support

- 📧 Email: hamzamohdzubair@gmail.com
- 🐛 Issues: [GitHub Issues](https://github.com/hamzamohdzubair/zen/issues)
- 💬 For account/billing: support@forgetmeifyoucan.com

---

**Happy learning!** 🚀
