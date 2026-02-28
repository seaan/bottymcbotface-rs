CREATE TABLE IF NOT EXISTS dnd_months (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id TEXT NOT NULL,
    month TEXT NOT NULL,
    title TEXT,
    access_token TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    UNIQUE(guild_id, month)
);

CREATE TABLE IF NOT EXISTS dnd_participants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    month_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    edit_token TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(month_id, name),
    FOREIGN KEY(month_id) REFERENCES dnd_months(id)
);

CREATE TABLE IF NOT EXISTS dnd_login_codes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    participant_id INTEGER NOT NULL,
    code TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    used_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(participant_id, code),
    FOREIGN KEY(participant_id) REFERENCES dnd_participants(id)
);

CREATE TABLE IF NOT EXISTS dnd_availability (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    participant_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(participant_id, date),
    FOREIGN KEY(participant_id) REFERENCES dnd_participants(id)
);
