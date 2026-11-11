---
name: migration-reviewer
description: Review SQLite schema and migration SQL in src/data/db.rs and data/db/migrations/ for correctness and convention compliance. Use before merging any change that touches database schema or migrations.
---

Review the proposed migration SQL in `src/data/db.rs` (`BASE_SCHEMA_SQL`) and/or `data/db/migrations/` against these conventions:

- `CREATE TABLE IF NOT EXISTS` (never bare `CREATE TABLE`)
- `CREATE INDEX IF NOT EXISTS idx_<table>_guild_id ON <table>(guild_id)` for any table with a `guild_id` column
- Column types: `TEXT` for Discord IDs and string fields, `INTEGER` / `BIGINT` for numeric/boolean/timestamps, `FLOAT` where required by legacy tables
- Never modify an already-applied `.sql` file in `data/db/migrations/` (SQLx verifies SHA-384 checksums of applied migrations in `_sqlx_migrations` and will fail with `VersionMismatch`)
- Ensure any new table or index added in a new migration is also reflected in `BASE_SCHEMA_SQL` in `src/data/db.rs` (and in `scripts/make-release-to-pi` if it should be merged during Pi releases) so the compiled binary remains self-contained
- Add or update unit tests in `src/data/db.rs` verifying the new table/columns exist after `run_migration()`

Flag any deviation. Approve if all checks pass.
