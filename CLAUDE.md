# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                                  # compile
cargo run                                    # run the bot (requires .env — see Environment below)
cargo test                                   # run all tests
cargo test <test_name>                       # run a single test by name
cargo test -- --test-threads=1               # run tests serially (needed when BOT_ENV is mutated)
cargo clippy --all-targets -- -D warnings    # lint
cargo fmt                                    # format
./scripts/install-git-hooks                  # install pre-commit hook (.githooks/pre-commit)
./scripts/make-release-to-pi                 # cross-compile, merge prod DB, and deploy to Raspberry Pi
```

## Environment

Requires a `.env` file in the project root. Key variables:

| Variable | Purpose |
|---|---|
| `DISCORD_TOKEN` | Bot token (required) |
| `BOT_ENV` | Set to `production` to use `data/db/production.db` and production channels; any other value uses `data/db/dev.db` |
| `DND_WEB_BIND_ADDR` | Axum bind address (default: `0.0.0.0:8787`) |
| `DND_PUBLIC_BASE_URL` | Public URL for DnD calendar links (default: `http://localhost:8787`) |
| `GH_TOKEN` | Fine-grained GitHub PAT used by `scripts/pi-autoupdate` on the Pi (Metadata: read + Contents: read) |

Database files live in `data/db/`. `database_path_from_env()` in [`src/data/db.rs`](src/data/db.rs) selects `data/db/production.db` or `data/db/dev.db` based on `BOT_ENV`, auto-creates parent directories, and enables `create_if_missing(true)` and `PRAGMA foreign_keys = ON`. Self-contained idempotent schema migrations (`BASE_SCHEMA_SQL` in `src/data/db.rs`, plus any SQLx migrations in `data/db/migrations/`) run automatically on startup.

## Architecture

The bot runs two concurrent subsystems inside a single Tokio process:

1. **Discord bot** — built on [poise](https://github.com/serenity-rs/poise) over serenity.
2. **Web server** — an Axum HTTP server (`src/web/`) that exposes the D&D availability calendar UI and JSON API on port 8787.

### Shared state (`src/data/`)

`Data` ([`src/data.rs`](src/data.rs)) is the poise user-data struct passed to every command. It holds `Arc<Mutex<T>>` handles for each domain:

- `db` — `BotDatabase` wrapper around an SQLx SQLite pool; used by bestof, quotes, requests
- `bestof` — in-memory best-of message cache, backed by SQLite (`messages` table)
- `quotes` — quote store, backed by SQLite (`quotes` table)
- `requests` — feature request store, backed by SQLite (`requests` table)
- `dnd` — `DndRepository` holds its **own** `Pool<Sqlite>` clone directly (no extra Mutex), shared with the Axum web server via `Arc` (`dnd_months`, `dnd_participants`, `dnd_login_codes`, `dnd_availability` tables)
- `quotes_for_response` — quote pool loaded from `data/k2so.txt` used for mention-response events

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

Unit and integration tests live inline in their respective modules (e.g., `src/data/db.rs`, `src/data/dnd.rs`, `src/commands/dnd_cmds.rs`, `src/web/dnd.rs`). DB tests use SQLite in-memory pools or temporary directories (`std::env::temp_dir()`).

### Gotchas

- **Never modify applied `.sql` files in `data/db/migrations/`** — SQLx records SHA-384 checksums in `_sqlx_migrations` and will fail with `VersionMismatch` on existing databases. Add new tables/columns idempotently in `BASE_SCHEMA_SQL` in `src/data/db.rs` (or as a new numbered migration file).
- **Tests use `std::env::temp_dir()`, not hardcoded `/tmp`** — honors `$TMPDIR` in sandboxed environments and falls back to `/tmp` outside.
- **`cargo test` accepts a single test-name filter** — use prefix matching (`cargo test data::db::`) and `-- --test-threads=1` when running env-var tests. This crate has no library target; use `cargo test --bin bottymcbotface-rs ...`, not `--lib`.
- **Poise `rename` must use `[a-z0-9_-]`** — Discord rejects spaces in command/option names at startup.
- **Two bot instances on same token = split gateway** — stop the Pi bot before running locally against the same token: `ssh pi "sudo systemctl stop bottymcbotface"`.

### CI / Releases

GitHub Actions (`.github/workflows/rust.yml`) runs two parallel build jobs on every push to `main`:
- `build` — static `x86_64-unknown-linux-musl` binary (artifact: `bottymcbotface-rs`)
- `build-aarch64` — static `aarch64-unknown-linux-musl` binary via `cross` (artifact: `bottymcbotface-rs-aarch64`)

The `release` job waits for both, then publishes a GitHub Release (`v<run_number>`) with both binaries attached.

### Deploying to Pi

`scripts/make-release-to-pi` cross-compiles for `aarch64-unknown-linux-musl` via `cross`, backs up and merges the local and Pi `production.db` SQLite databases, verifies integrity with `PRAGMA quick_check;`, and rsyncs the binary, static assets (`src/web/assets/`, `data/k2so.txt`), `scripts/pi-autoupdate`, and systemd unit files to the Pi, then installs and restarts them:

- Binary, assets, and DB land in `/home/pi/bot/release/`
- `.env` must exist at `/home/pi/bot/release/.env` on the Pi
- `bottymcbotface.service` is installed to `/etc/systemd/system/bottymcbotface.service`
- `bottymcbotface-update.service` + `bottymcbotface-update.timer` are installed and the timer is enabled

#### Pi deploy gotchas

- **DB ownership after rsync** — the service runs as `bot:bot`. `make-release-to-pi` runs `sudo chown bot:bot` automatically, but if deploying manually, chown `production.db` and `data/` to `bot:bot`.
- **SC2029 in `make-release-to-pi`** — `ssh pi "...${VAR}..."` calls use intentional client-side expansion; new ones need `# shellcheck disable=SC2029`.
- **`gh` CLI auth for `bot` user** — `pi-autoupdate` uses `gh` CLI. Add `GH_TOKEN=<fine-grained-pat>` to `/home/pi/bot/release/.env` so `bottymcbotface-update.service` can authenticate via `EnvironmentFile`.

### Auto-update on Pi

A systemd timer (`bottymcbotface-update.timer`) runs `scripts/pi-autoupdate` every 5 minutes (first run 2 min after boot). The script:
1. Uses `gh` CLI to get the latest release tag from GitHub (`seaan/bottymcbotface-rs`)
2. Compares to `/home/pi/bot/release/version` (absent → always updates)
3. Downloads `bottymcbotface-rs-aarch64`, verifies it is non-empty, stops `bottymcbotface`, swaps the binary, writes the version file, and restarts `bottymcbotface`

**One-time Pi setup required** (sudoers):
```
sudo visudo -f /etc/sudoers.d/bot-systemctl
# Add:
bot ALL=(root) NOPASSWD: /bin/systemctl stop bottymcbotface, /bin/systemctl start bottymcbotface
```

Check update logs: `journalctl -u bottymcbotface-update.service -n 50`
