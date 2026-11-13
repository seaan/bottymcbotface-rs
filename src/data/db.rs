use log::info;
use sqlx::{
    Pool, Sqlite,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{env::var, path::Path, str::FromStr};

const BASE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER NOT NULL PRIMARY KEY,
    author TEXT,
    content TEXT,
    link TEXT,
    channel TEXT,
    count BIGINT,
    timestamp FLOAT,
    image TEXT,
    last_posted_at TEXT
);

CREATE INDEX IF NOT EXISTS ix_messages_4f0aa52d656a3d75 ON messages (link);

CREATE TABLE IF NOT EXISTS meirl (
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    image TEXT,
    link TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    channel TEXT,
    count BIGINT,
    CONSTRAINT uq_messages UNIQUE(author, link)
);

CREATE INDEX IF NOT EXISTS ix_meirl_4f0aa52d656a3d75 ON meirl (link);

CREATE TABLE IF NOT EXISTS quotes (
    id INTEGER NOT NULL PRIMARY KEY,
    quote TEXT,
    author TEXT
);

CREATE TABLE IF NOT EXISTS requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request TEXT NOT NULL,
    user TEXT NOT NULL,
    votes INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS dnd_months (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id TEXT NOT NULL,
    month TEXT NOT NULL,
    title TEXT,
    access_token TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    UNIQUE(guild_id, month)
);

CREATE INDEX IF NOT EXISTS idx_dnd_months_guild_id ON dnd_months(guild_id);

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
"#;

pub fn database_path_from_env() -> String {
    match var("BOT_ENV") {
        Ok(env) if env == "production" => {
            info!("Using production database");
            "data/db/production.db".to_string()
        }
        _ => {
            info!("Using development database");
            "data/db/dev.db".to_string()
        }
    }
}

fn ensure_parent_dir(db_path: &str) -> std::io::Result<()> {
    if db_path != ":memory:"
        && let Some(parent) = Path::new(db_path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub struct BotDatabase {
    conn: Pool<Sqlite>,
}

impl BotDatabase {
    pub fn new() -> BotDatabase {
        let db_path = database_path_from_env();
        Self::new_with_path(&db_path)
    }

    pub fn new_with_path(db_path: &str) -> BotDatabase {
        ensure_parent_dir(db_path)
            .unwrap_or_else(|e| panic!("Failed to create parent directory for {db_path:?}: {e}"));

        let options = if db_path == ":memory:" {
            SqliteConnectOptions::from_str("sqlite::memory:")
                .expect("valid sqlite memory url")
                .foreign_keys(true)
        } else {
            SqliteConnectOptions::new()
                .filename(db_path)
                .create_if_missing(true)
                .foreign_keys(true)
        };

        let new_connection = if db_path == ":memory:" {
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_lazy_with(options)
        } else {
            Pool::connect_lazy_with(options)
        };

        BotDatabase {
            conn: new_connection,
        }
    }

    pub async fn run_migration(&self) -> Result<(), sqlx::Error> {
        use sqlx::migrate::Migrator;

        sqlx::raw_sql(BASE_SCHEMA_SQL).execute(&self.conn).await?;

        // Incremental migrations for existing databases (no-op if column already exists)
        let _ = sqlx::query("ALTER TABLE messages ADD COLUMN last_posted_at TEXT")
            .persistent(false)
            .execute(&self.conn)
            .await;

        let migrations_path = Path::new("./data/db/migrations");
        if migrations_path.exists() {
            let migrator = Migrator::new(migrations_path).await?;
            migrator.run(&self.conn).await?;
        } else {
            info!("Skipping external migrations directory: ./data/db/migrations does not exist.");
        }

        Ok(())
    }

    /// Get a reference to the connection pool
    pub fn get_conn(&self) -> &Pool<Sqlite> {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn database_path_uses_dev_by_default() {
        let _guard = env_lock().lock().unwrap();
        unsafe { std::env::remove_var("BOT_ENV") };
        assert_eq!(database_path_from_env(), "data/db/dev.db");
    }

    #[test]
    fn database_path_uses_production_when_bot_env_is_production() {
        let _guard = env_lock().lock().unwrap();
        unsafe { std::env::set_var("BOT_ENV", "production") };
        assert_eq!(database_path_from_env(), "data/db/production.db");
        unsafe { std::env::remove_var("BOT_ENV") };
    }

    #[tokio::test]
    async fn new_with_path_creates_parent_directory_and_db_file() {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("bottymcbotface-db-dir-{}", nanos));
        let db_path = dir.join("nested/test.db");

        let db = BotDatabase::new_with_path(db_path.to_string_lossy().as_ref());
        db.run_migration().await.unwrap();

        assert!(db_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn migrations_create_all_core_tables_idempotently() {
        let db = BotDatabase::new_with_path(":memory:");
        db.run_migration().await.unwrap();
        // Running a second time must be a clean no-op
        db.run_migration().await.unwrap();

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master
             WHERE type = 'table'
               AND name IN (
                   'messages',
                   'meirl',
                   'quotes',
                   'requests',
                   'dnd_months',
                   'dnd_participants',
                   'dnd_login_codes',
                   'dnd_availability'
               )
             ORDER BY name",
        )
        .fetch_all(db.get_conn())
        .await
        .unwrap();

        let names: Vec<String> = rows.into_iter().map(|(n,)| n).collect();
        assert_eq!(
            names,
            vec![
                "dnd_availability",
                "dnd_login_codes",
                "dnd_months",
                "dnd_participants",
                "meirl",
                "messages",
                "quotes",
                "requests",
            ]
        );
    }

    #[tokio::test]
    async fn migrations_add_last_posted_at_to_existing_messages_table() {
        let db = BotDatabase::new_with_path(":memory:");
        sqlx::query(
            "CREATE TABLE messages (
                id INTEGER NOT NULL PRIMARY KEY,
                author TEXT,
                content TEXT,
                link TEXT,
                channel TEXT,
                count BIGINT,
                timestamp FLOAT,
                image TEXT
            )",
        )
        .execute(db.get_conn())
        .await
        .unwrap();

        db.run_migration().await.unwrap();

        sqlx::query(
            "INSERT INTO messages (id, author, content, link, channel, count, timestamp, image, last_posted_at)
             VALUES (1, 'alice', 'hello', 'https://discord.com', 'general', 5, 1700000000.0, NULL, '2026-11-11T15:00:00+00:00')",
        )
        .execute(db.get_conn())
        .await
        .unwrap();

        let (last_posted_at,): (Option<String>,) =
            sqlx::query_as("SELECT last_posted_at FROM messages WHERE id = 1")
                .fetch_one(db.get_conn())
                .await
                .unwrap();
        assert_eq!(last_posted_at.as_deref(), Some("2026-11-11T15:00:00+00:00"));
    }

    #[tokio::test]
    async fn load_bestof_from_file_db_after_adding_last_posted_at_column() {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("bottymcbotface-mig-load-{}", nanos));
        let db_path = dir.join("test.db");

        let mut db = BotDatabase::new_with_path(db_path.to_string_lossy().as_ref());
        sqlx::query(
            "CREATE TABLE messages (
                id INTEGER NOT NULL PRIMARY KEY,
                author TEXT,
                content TEXT,
                link TEXT,
                channel TEXT,
                count BIGINT,
                timestamp FLOAT,
                image TEXT
            )",
        )
        .execute(db.get_conn())
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO messages (id, author, content, link, channel, count, timestamp, image)
             VALUES (1, 'alice', 'hello', 'https://discord.com', 'general', 5, 1700000000.0, NULL)",
        )
        .execute(db.get_conn())
        .await
        .unwrap();

        db.run_migration().await.unwrap();

        let mut bestof = crate::data::bestof::BestOf::new();
        bestof.load_from_persisted_db(&mut db).await.unwrap();
        assert!(bestof.pick_daily_bestof_at(chrono::Utc::now()).is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
