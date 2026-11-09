# tiktok-ttsk

![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)
![Stealth](https://img.shields.io/badge/browser-CloakBrowser%20via%20cloak--cdp-purple)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![CLI](https://img.shields.io/badge/interface-CLI-informational)

**tiktok-ttsk** (TikTok Streak Keeper) is a Rust command-line tool that helps maintain TikTok conversation streaks by sending a daily message to your chats through a stealth browser session.

## Overview

The tool uses [`cloak-cdp`](https://crates.io/crates/cloak-cdp) to drive CloakHQ's CloakBrowser patched Chromium binary over CDP, reuses a saved login session, opens TikTok Messages, and sends a randomly selected message to each conversation. It stores local state so the daily run only happens once per local day.

## Features

- **Stealth browser automation** via `cloak-cdp` (CloakBrowser source-patched Chromium binary over CDP)
- **Human-like input** with per-character typing delays and robust message sending (send button / key fallback)
- **Daily guard** based on local `YYYY-MM-DD` state
- **Persistent login** using browser cookies saved as JSON
- **Configurable messages** via a TOML config file
- **CLI commands**: `run`, `login`, `status`, and `init-config`
- **Safe preview mode** with `--dry-run`
- **Operational controls** like `--limit`, `--force`, and `--headless`
- **Desktop notifications** and file logging in the application data directory

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- Network access on first run so `cloak-cdp` can download/verify the required stealth Chromium binary (unless you provide a local binary)

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
| `config.toml` | User configuration (messages, URL, delays, browser args/path) |
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
browser_path = ""
```

To use a local browser binary instead of the managed download, set `browser_path` in config, pass `--browser-path`, or set `CLOAKBROWSER_BINARY_PATH`.

## How it works

1. Loads configuration and local state from the data directory.
2. Verifies that a saved session exists (or asks you to run `login`).
3. Launches the stealth CloakBrowser binary via `cloak-cdp` and restores cookies.
4. Opens TikTok Messages and finds conversation items.
5. Types one random configured message per conversation and sends it (unless `--dry-run` is set).
6. Saves the current local date to prevent duplicate runs on the same day.

## Notes

- Browser automation depends on TikTok's current web UI selectors; if TikTok changes its interface, selectors may need updates.
- `cloak-cdp` is an unofficial Rust integration that uses CloakHQ's CloakBrowser binary/technology; comply with CloakHQ's binary/license terms when using it.
- Use `--dry-run` when testing configuration or selector changes.
- Automating interactions may conflict with TikTok's Terms of Service. Use responsibly.
