# Copilot Instructions for AI Coding Agents

## Project Overview
This is a Rust-based Discord bot (`bottymcbotface-rs`) built with `poise` over `serenity`, `tokio`, `sqlx` (SQLite), and `axum`. It runs a Discord bot alongside an Axum HTTP server for D&D availability scheduling.

## Architecture & Major Components
- **`src/main.rs`**: Entry point, Poise framework setup, command registration, database migration trigger, scheduled task spawning, and Axum server startup.
- **`src/commands/`**: Slash and prefix command handlers grouped by domain (`bestof_cmds.rs`, `quote_cmds.rs`, `request_cmds.rs`, `gamenight_cmds.rs`, `dnd_cmds.rs`).
- **`src/events/`**: Event listeners (`mentionme.rs` responds to @mentions with quotes from `data/k2so.txt`).
- **`src/data/`**: SQLite persistence (`db.rs`) and domain repositories (`bestof.rs`, `quotes.rs`, `requests.rs`, `dnd.rs`).
  - `database_path_from_env()` in `src/data/db.rs` selects `data/db/production.db` when `BOT_ENV=production` and `data/db/dev.db` otherwise.
  - Idempotent schema initialization (`BASE_SCHEMA_SQL` in `src/data/db.rs`) runs automatically on startup.
- **`src/web/`**: Axum HTTP server (`mod.rs`, `dnd.rs`) serving the D&D availability calendar UI and JSON API (`src/web/assets/`).
- **`src/scheduled.rs`**: Background tasks for daily best-of, daily quote, reaction scanning, and SQLite sync.
- **`src/constants.rs`**: Channel routing constants (`get_update_channel_id()` switches on `BOT_ENV`).
- **`scripts/`**:
  - `scripts/install-git-hooks` — configures `.githooks/pre-commit` (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`).
  - `scripts/make-release-to-pi` — cross-compiles `aarch64-unknown-linux-musl`, backs up and merges `production.db`, and deploys to the Raspberry Pi.
  - `scripts/pi-autoupdate` — systemd-driven auto-updater polling GitHub Releases.

## Developer Workflows
- **Build**: `cargo build`
- **Format**: `cargo fmt`
- **Lint**: `cargo clippy --all-targets -- -D warnings`
- **Test**: `cargo test -- --test-threads=1`
- **Deploy to Pi**: `./scripts/make-release-to-pi`

## Conventions & Safety Rules
- Always use parameterized SQL queries (`sqlx::query` / `sqlx::query_as` with `.bind(...)`), never `format!(...)` with user input.
- Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for schema additions in `src/data/db.rs`. Do not edit already-applied migration files in `data/db/migrations/`.
- Keep `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean before committing.
