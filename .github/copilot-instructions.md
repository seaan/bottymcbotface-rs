# Copilot Instructions for AI Coding Agents

## Project Overview
This is a Rust-based Discord bot. The codebase is organized for modularity and maintainability, with clear separation of commands, events, and data management. The main entry point is `src/main.rs`.

## Architecture & Major Components
- **src/commands/**: Command handlers, grouped by feature (e.g., `bestof_cmds.rs`, `quote_cmds.rs`).
- **src/events/**: Event listeners (e.g., `mentionme.rs`).
- **src/data/**: Data access and persistence logic (e.g., `db.rs`, `quotes.rs`).
- **src/constants.rs**: Centralized constants for configuration and shared values.
- **src/scheduled.rs**: Scheduled/background tasks.
- **data/**: External data files (e.g., `k2so.txt`).

## Developer Workflows
- **Build**: Use `cargo build`.
- **Run**: Use `cargo run` to start the bot.
- **Release**: See `scripts/make-release-to-pi` for deployment to Raspberry Pi.
- **No standard test suite detected**: If tests are added, document their location and invocation.

## Project-Specific Conventions
- **Command/Event Separation**: Each command/event type has its own file for clarity and isolation.
- **Feature Grouping**: Related commands/events are grouped in subdirectories (e.g., `commands/`, `events/`, `data/`).
- **Data Files**: External data is stored in the `data/` directory, loaded as needed.
- **Constants**: Use `src/constants.rs` for shared values.

## Integration Points & Dependencies
- **Discord API (Serenity crate)**: The bot interacts with Discord using the `serenity` Rust crate. Understanding Serenity's event, command, and context models is crucial for working with this codebase (see `main.rs` for setup and registration).
- **External Scripts**: Deployment uses shell scripts in `scripts/`.
- **No database detected**: Data is likely file-based (see `src/data/`).

## Examples
- To add a new command: Create a file in `src/commands/`, implement the handler, and register it in `main.rs`.
- To add a new event: Add a file in `src/events/` and hook it up in `main.rs`.
- To persist new data: Extend the appropriate module in `src/data/` and update data files in `data/`.

## Key Files
- `src/main.rs`: Entry point, bot setup, command/event registration.
- `src/commands/`: Command handlers.
- `src/events/`: Event listeners.
- `src/data/`: Data management.
- `src/constants.rs`: Shared constants.
- `scripts/make-release-to-pi`: Deployment script.
- `data/`: External data files.

---

If any conventions or workflows are unclear or missing, please provide feedback to improve these instructions.