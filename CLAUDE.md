# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run (requires .env file — see Environment below)
cargo run

# Run tests
cargo test

# Run a single test by name
cargo test <test_name>

# Cross-compile and deploy to Raspberry Pi
./scripts/make-release-to-pi
```

## Environment

Requires a `.env` file in the project root. Key variables:

| Variable | Purpose |
|---|---|
| `DISCORD_TOKEN` | Bot token (required) |
| `BOT_ENV` | Set to `production` to use production DB and channels; any other value uses dev |
| `DND_WEB_BIND_ADDR` | Axum bind address (default: `0.0.0.0:8787`) |
| `DND_PUBLIC_BASE_URL` | Public URL for DnD calendar links (default: `http://localhost:8787`) |

Database files live in `data/db/`. The bot selects `data/db/production.db` or `data/db/dev.db` based on `BOT_ENV`. SQLx migrations in `data/db/migrations/` run automatically on startup.

## Architecture

The bot is built on [poise](https://github.com/serenity-rs/poise) (Discord command framework) over serenity, with tokio as the async runtime.

### Shared state (`src/data/`)

`Data` ([src/data.rs](src/data.rs)) is the poise user-data struct passed to every command. It holds `Arc<Mutex<T>>` handles for each domain:

- `db` — `BotDatabase` wrapper around an SQLx SQLite pool; used by bestof, quotes, requests
- `bestof` — in-memory best-of message cache, backed by SQLite
- `quotes` — quote store, backed by SQLite
- `requests` — feature request store, backed by SQLite
- `dnd` — `DndRepository` holds its **own** `Pool<Sqlite>` clone directly (no extra Mutex), shared with the Axum web server via `Arc`
- `quotes_for_response` — quote pool used for mention-response events

### Slash commands (`src/commands/`)

Each domain has its own file:

| File | Command group |
|---|---|
| `bestof_cmds.rs` | `/bestof` — manage and retrieve best-of messages |
| `quote_cmds.rs` | `/quote` — add/retrieve community quotes |
| `request_cmds.rs` | `/request` — submit feature requests |
| `gamenight_cmds.rs` | `/gamenight` — game night coordination |
| `dnd_cmds.rs` | `/dnd submit|status|results` — D&D session scheduling |

New commands must be registered in the `commands` vec in [`src/main.rs`](src/main.rs).

### Event handling (`src/events/`)

`handle_event` in [`src/events.rs`](src/events.rs) routes Discord events. Currently only `Message` events are handled — if the bot is @mentioned, it responds with a quote via `events/mentionme.rs`.

### Scheduled tasks (`src/scheduled.rs`)

Four background tasks spawned at startup:

- **Daily bestof** — posts a random best-of embed at 15:00 UTC
- **Daily quote** — posts a random quote at 15:00:05 UTC
- **Reaction scanner** — polls for new best-of candidates every 10 minutes (messages with ≥5 reactions)
- **DB sync** — persists bestof in-memory state to SQLite every ~hour

### Web server (`src/web/`)

An Axum HTTP server runs concurrently with the Discord bot on port 8787. It exposes a D&D availability calendar UI and JSON API (`src/web/dnd.rs`). Static assets are served from `src/web/assets/`. The server shares `Arc<DndRepository>` with the bot without going through the poise `Data` struct.

### Channel routing

`src/constants.rs` defines `get_update_channel_id()` — returns the production `#quotes` channel or a dev DM channel based on `BOT_ENV`. Scheduled posts go here.

### Testing

Unit and integration tests live inline in their respective modules (e.g., `src/data/dnd.rs`, `src/commands/dnd_cmds.rs`). DB tests for `DndRepository` use SQLite in-memory pools with a schema bootstrapped via `init_for_tests()`.

### Deployment

Production runs on a Raspberry Pi (`aarch64`). `scripts/make-release-to-pi` cross-compiles with `cross` and rsync-deploys the binary, then restarts the `bottymcbotface` systemd service.
