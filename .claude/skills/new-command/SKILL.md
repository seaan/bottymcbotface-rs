---
name: new-command
description: Scaffold a new poise slash command in bottymcbotface-rs. Use when the user wants to add a Discord slash command. Follows the existing command patterns in src/commands/.
---

# New Poise Slash Command

Scaffold a new slash command following the `bottymcbotface-rs` conventions.

## Steps

1. Read `src/commands.rs` and existing domain command files in `src/commands/` (e.g. `src/commands/dnd_cmds.rs`, `src/commands/quote_cmds.rs`) to understand the command signature, `#[poise::command]` attributes, `Context<'_>` usage, and error handling.
2. Add the new command function (or command group with `subcommands(...)`) in a domain module under `src/commands/<domain>_cmds.rs` and export it in `src/commands.rs` (or add simple top-level commands directly in `src/commands.rs`).
3. Register the top-level command in the `commands` vec in `src/main.rs`.
4. If the command needs DB access, access `ctx.data().db` (or the relevant domain struct on `ctx.data()`) and add any new tables/columns to `BASE_SCHEMA_SQL` in `src/data/db.rs`.

## Command template

```rust
use crate::{Context, Error};

/// One-line description shown in Discord
#[poise::command(slash_command, prefix_command)]
pub async fn command_name(
    ctx: Context<'_>,
    #[description = "Param description"] param: String,
) -> Result<(), Error> {
    ctx.say(format!("Received: {param}")).await?;
    Ok(())
}
```

## Reminders

- Poise `rename` attributes and option names must match `[a-z0-9_-]` (no spaces).
- Guild-scoped commands can require `guild_only` or read `ctx.guild_id().ok_or("Command must be used in a server")?`.
- Commands are registered globally at startup via `poise::builtins::register_globally` in `src/main.rs`.
- After adding, confirm the command appears in the `commands` vec passed to `poise::FrameworkOptions` in `src/main.rs`.
