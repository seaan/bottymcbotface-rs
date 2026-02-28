# DnD Month Planner Design

## Summary
Build a Discord-native DnD scheduling flow with a lightweight web calendar for date selection.
Discord remains the control plane; the web UI exists only to make date picking fast (click and drag).

## Product Goals
- Primary UX in Discord commands.
- No organizer setup required; month planning is rolling and self-service.
- Availability entry on a web calendar with date-level granularity only.
- Anonymous participant names (no Discord OAuth).
- Per-day autosave so users can return and update one day at a time.
- Live, query-time results for top two dates.

## Non-Goals (V1)
- Year-aware month planning across multiple years.
- Time-slot scheduling.
- Manual round closing.
- Persistent user accounts beyond month+name identity.

## User Flow
1. Participant runs `/dnd submit <month> <name>`.
2. Bot creates the month record if missing and creates/loads the participant.
3. Bot returns an ephemeral login code (reusable until expiry) and month web link.
4. Participant opens the web page, enters month, name, and code.
5. Web session issues an edit token and loads previous selections.
6. Participant click-drags days; each change autosaves immediately.
7. Anyone with permissions runs `/dnd results <month>` at any time to get the current top 2 dates.

## Command Surface
- `/dnd submit month:<month_name> name:<string>`
  - Auto-creates month record if absent for the guild.
  - Creates/loads participant and issues short-lived login code.
  - Response is ephemeral.
- `/dnd status month:<month_name>`
  - Returns participation count and coverage snapshot.
- `/dnd results month:<month_name>`
  - Returns top 2 dates by highest distinct participant availability.

## Month Semantics
- Input format is month name only: `january`..`december`.
- V1 stores month as a canonical enum/string key only, deferring year support.
- Scope is per server: one logical month record per (`guild_id`, `month`).
- Read-only is enforced when the month is over according to local server date logic.
  - In read-only mode: login still works for viewing; write endpoints reject updates.

## Architecture
Single Rust process:
- Existing Poise/Serenity Discord bot runtime.
- Added HTTP server runtime (Axum) for DnD endpoints and static assets.
- Shared SQLite database pool for bot commands and web handlers.

Modules:
- `src/commands/dnd_cmds.rs`: `/dnd` command group and subcommands.
- `src/data/dnd.rs`: data access layer for month/participant/availability/auth codes.
- `src/web/dnd.rs`: HTTP handlers for login/session/autosave/summary.
- `src/web/mod.rs`: server setup and route mounting.
- `src/data.rs`, `src/main.rs`, `src/commands.rs`: wiring and startup.

## Data Model
### `dnd_months`
- `id` INTEGER PK
- `guild_id` TEXT NOT NULL
- `month` TEXT NOT NULL
- `title` TEXT NULL
- `access_token` TEXT NOT NULL UNIQUE
- `created_at` INTEGER NOT NULL
- UNIQUE (`guild_id`, `month`)

### `dnd_participants`
- `id` INTEGER PK
- `month_id` INTEGER NOT NULL FK
- `name` TEXT NOT NULL
- `edit_token` TEXT NOT NULL UNIQUE
- `created_at` INTEGER NOT NULL
- `updated_at` INTEGER NOT NULL
- UNIQUE (`month_id`, `name`)

### `dnd_login_codes`
- `id` INTEGER PK
- `participant_id` INTEGER NOT NULL FK
- `code` TEXT NOT NULL
- `expires_at` INTEGER NOT NULL
- `created_at` INTEGER NOT NULL
- `used_count` INTEGER NOT NULL DEFAULT 0
- UNIQUE (`participant_id`, `code`)

### `dnd_availability`
- `id` INTEGER PK
- `participant_id` INTEGER NOT NULL FK
- `date` TEXT NOT NULL (`YYYY-MM-DD`)
- `created_at` INTEGER NOT NULL
- UNIQUE (`participant_id`, `date`)

## Auth + Session Model
- `/dnd submit` issues an 8-character login code that expires (for example 15 minutes).
- Codes are reusable until expiration.
- Web login requires month + name + code.
- On successful login, backend returns participant `edit_token`.
- All write APIs require `edit_token`; code is only for session establishment.

## Web API
- `GET /dnd/<month_token>`
  - Returns the DnD calendar page.
- `POST /api/dnd/login`
  - Input: `{ month, name, code }`
  - Output: `{ edit_token, selected_dates[], readonly }`
- `PATCH /api/dnd/availability`
  - Input: `{ edit_token, set_available[], set_unavailable[] }`
  - Output: `{ ok, selected_count }`
- `GET /api/dnd/summary?month=<month>`
  - Output: per-date counts and participant count.

## Ranking
Results query uses distinct participants per date:
- Sort by `count DESC`, then `date ASC`.
- `/dnd results` returns top two entries from this ordered list.

## Error Handling
- Invalid month name in commands/login returns a clear guidance message.
- Name collision is allowed only with valid login code; otherwise denied.
- Expired/invalid code returns auth error; user can request a new code via `/dnd submit`.
- Expired month returns read-only on web writes.

## Testing Strategy
- Unit tests: month parsing and read-only cutoff logic.
- Data tests: participant upsert, login code validation, date insert/delete idempotency.
- Command tests: `/dnd submit` auto-create behavior, `/dnd results` empty and populated cases.
- HTTP tests: login success/failure and autosave patch behavior.

## Rollout
1. Add schema migration and data layer first.
2. Add `/dnd` commands with rolling month auto-create and token/code issuance.
3. Add web handlers and page UI with drag-select calendar.
4. Verify with local SQLite and end-to-end command flow.
