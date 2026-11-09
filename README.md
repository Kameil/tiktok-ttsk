# tiktok-ttsk

![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![CLI](https://img.shields.io/badge/interface-CLI-informational)
![Status](https://img.shields.io/badge/status-active-success)

**tiktok-ttsk** (TikTok Streak Keeper) is a command-line tool written in Rust that helps maintain your TikTok conversation streaks by sending a message to your chats once per day through a browser session.

## Overview

The tool launches Chromium, reuses a saved login session, opens TikTok Messages, and sends a randomly selected message to each conversation. It keeps local state so it only performs the daily run once per day, with options to preview, limit, or force execution.

## Features

- **Rust rewrite** with a modular codebase and async browser automation
- **Daily guard** based on local `YYYY-MM-DD` state (avoids day-of-month rollover issues)
- **Persistent login** using exported browser cookies stored as JSON
- **Configurable messages** via a TOML config file
- **CLI commands**: `run`, `login`, `status`, and `init-config`
- **Safe preview mode** with `--dry-run`
- **Operational controls** like `--limit`, `--force`, and `--headless`
- **Desktop notifications** for login and run status
- **File logging** in the application data directory

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- Google Chrome or Chromium installed and discoverable by the system

## Installation

### Build from source

```bash
git clone git@github.com:luqenov/tiktok-ttsk.git
cd tiktok-ttsk
cargo build --release
```

The compiled binary will be available at:

```text
target/release/tiktok-ttsk
```

## Usage

### 1. Save your TikTok session

```bash
cargo run -- login
```

A browser window opens on the TikTok login page. After you log in, the session cookies are saved locally.

### 2. Send daily streak messages

```bash
cargo run -- run
```

If messages were already sent today, the command exits without sending duplicates.

### Useful commands

```bash
# Preview without sending
cargo run -- run --dry-run

# Process only the first 10 conversations
cargo run -- run --limit 10

# Ignore the daily guard
cargo run -- run --force

# Show paths, state, and configuration summary
cargo run -- status

# Create a default config file
cargo run -- init-config
```

## Configuration

By default, data is stored in your platform's local application data directory under `tiktok-ttsk/` (override with `--data-dir` or `TIKTOK_TTSK_DATA_DIR`).

Files used by the app:

| File | Purpose |
| --- | --- |
| `config.toml` | User configuration (messages, URL, delays, browser args) |
| `cookies.json` | Saved browser cookies for the TikTok session |
| `state.json` | Last successful send date |
| `app.log` | Application logs |

Generate a default config:

```bash
cargo run -- init-config
```

Example `config.toml`:

```toml
messages = ["hey", "streak", "fogo", "sossegado"]
messages_url = "https://www.tiktok.com/messages?lang=en"
headless = false
browser_args = []
delay_between_ms = [1500, 3000]
typing_delay_ms = 120
```

## How it works

1. Loads configuration and local state from the data directory.
2. Verifies that a saved session exists (or asks you to run `login`).
3. Launches Chromium and restores cookies.
4. Opens TikTok Messages and finds conversation items.
5. Sends one random configured message per conversation (unless `--dry-run` is set).
6. Saves the current local date to prevent duplicate runs on the same day.

## Notes

- Browser automation depends on TikTok's current web UI selectors; if TikTok changes its interface, selectors may need updates.
- Use `--dry-run` after UI changes or when testing a new configuration.
- Automating interactions may conflict with TikTok's Terms of Service. Use responsibly.
