# QoL Port from `sadie-bot` — TODO

Checklist of Quality-of-Life, database, CI/CD, deployment, and developer experience improvements ported from `../sadie-bot` into `bottymcbotface-rs`.

## 1. Database Schema, Migrations & Prod/Dev Split (`src/data/db.rs`, `src/data/*.rs`)
- [x] Add `database_path_from_env()` selecting `data/db/production.db` when `BOT_ENV=production` and `data/db/dev.db` otherwise.
- [x] Auto-create parent directories (`std::fs::create_dir_all`) and configure SQLite connections with `create_if_missing(true)` and `PRAGMA foreign_keys = ON`.
- [x] Consolidate all tables (`messages`, `meirl`, `quotes`, `requests`, `dnd_months`, `dnd_participants`, `dnd_login_codes`, `dnd_availability`) and indexes into idempotent `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` migrations executed automatically at startup in `src/data/db.rs` (self-contained in the binary).
- [x] Remove redundant per-insert table init from `src/data/requests.rs` and replace raw SQL `format!(...)` string interpolation in `src/data/quotes.rs` and `src/data/requests.rs` with bound parameters (`?`).
- [x] Add unit tests in `src/data/db.rs` covering `BOT_ENV` path selection, parent directory creation, and idempotent schema creation for all tables.

## 2. Git Pre-Commit Hooks & Code Quality (`.githooks/pre-commit`, `scripts/install-git-hooks`)
- [x] Add `.githooks/pre-commit` to run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -- --test-threads=1` when Rust/Cargo files are staged.
- [x] Add `scripts/install-git-hooks` and configure `git config core.hooksPath .githooks`.
- [x] Fix existing formatting and Clippy warnings across the codebase so `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass cleanly.

## 3. CI/CD & Raspberry Pi Auto-Updates (`.github/workflows/rust.yml`, systemd units, `scripts/pi-autoupdate`)
- [x] Update `.github/workflows/rust.yml` to build static `x86_64-unknown-linux-musl` (`bottymcbotface-rs`) and `aarch64-unknown-linux-musl` (`bottymcbotface-rs-aarch64` via `cross`) in parallel with `Swatinem/rust-cache@v2`, and attach both to GitHub Releases via `softprops/action-gh-release@v2`.
- [x] Add `bottymcbotface.service`, `bottymcbotface-update.service`, and `bottymcbotface-update.timer`.
- [x] Add `scripts/pi-autoupdate` to poll GitHub Releases via `gh`, download `bottymcbotface-rs-aarch64`, swap the binary, and restart `bottymcbotface`.

## 4. Safer Pi Deployment & Production DB Merge (`scripts/make-release-to-pi`)
- [x] Update `scripts/make-release-to-pi` to:
  - Cross-compile static `aarch64-unknown-linux-musl` binary.
  - Validate required CLI tools (`cross`, `rsync`, `ssh`, `sqlite3`).
  - Create timestamped backups of local and Pi `production.db` in `data/db/backups/`.
  - Pull Pi `production.db`, ensure all schema tables exist, and merge local `production.db` rows safely without losing Pi data.
  - Validate merged DB integrity with `PRAGMA quick_check;`.
  - Sync merged DB, static assets (`src/web/assets/`, `data/k2so.txt`), binary, `pi-autoupdate`, and systemd `.service`/`.timer` units to the Pi, fix `bot:bot` ownership, and restart services.

## 5. Claude / AI Tooling, Guardrails, Skills & Docs (`.claude/`, `CLAUDE.md`, `.github/copilot-instructions.md`)
- [x] Add `.claude/settings.json` with `PreToolUse` safety hooks (block `.env` edits, warn on `make-release-to-pi`) and `PostToolUse` hooks (`cargo clippy`, `cargo fmt`, and targeted module tests on `.rs` edits).
- [x] Add `.claude/skills/new-command/SKILL.md` tailored to `bottymcbotface-rs`'s `src/commands/` layout.
- [x] Add `.claude/agents/migration-reviewer.md` and `.claude/agents/security-reviewer.md` tailored to `bottymcbotface-rs`.
- [x] Update `CLAUDE.md` and `.github/copilot-instructions.md` with up-to-date architecture, DB migration rules, CI/CD, Pi deployment, and auto-updater gotchas.

## 6. Dependency Pinning & Config Hygiene (`Cargo.toml`, `.gitignore`)
- [x] Update `.gitignore` to ignore `data/db/*.db*`, `data/db/backups/`, `*.db`, and `.claude/settings.local.json`.
## 7. Daily BestOf Smart Selection & Recency Cooldown (`src/data/bestof.rs`, `src/scheduled.rs`)
- [x] Add `last_posted_at` column to `messages` table (with idempotent migration in `src/data/db.rs`).
- [x] Implement 3-tier daily bestof selection (`SameDay` anniversary -> `SameWeek` anniversary -> `Random` fallback), weighted by `days_since_posted^2` with a 7-day cooldown window.
- [x] Persist `last_posted_at` in memory and SQLite when `post_daily_bestof` posts a message, and add unit tests covering tier priority, recency weighting, cooldown skipping, and DB persistence.
