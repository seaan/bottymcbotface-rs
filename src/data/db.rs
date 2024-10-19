use log::info;
use sqlx::{Pool, Sqlite};
use std::env::var;

pub struct BotDatabase {
    conn: Pool<Sqlite>,
}

impl BotDatabase {
    pub fn new() -> BotDatabase {
        let db_url = BotDatabase::get_database_url();
        let new_connection = Pool::connect_lazy(&db_url)
            .unwrap_or_else(|_| panic!("Failed to connect to database {:?}", db_url));

        BotDatabase {
            conn: new_connection,
        }
    }

    /// Determine the correct database URL based on the environment
    fn get_database_url() -> String {
        match var("BOT_ENV") {
            Ok(env) if env == "production" => {
                info!("Using production database");
                // Return the production database URL
                "sqlite://data/db/production.db".to_string()
            }
            _ => {
                // Default to development/testing database URL
                info!("Using development database");
                "sqlite://data/db/dev.db".to_string()
            }
        }
    }

    pub async fn run_migration(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./data/db/migrations")
            .run(&self.conn)
            .await?;
        Ok(())
    }

    /// Load all rows from any given table.
    pub async fn load_all_from_table<T>(
        &self,
        table_name: &str,
    ) -> Result<Vec<T>, Box<dyn std::error::Error>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        let query = format!("SELECT * FROM {}", table_name);

        let rows = sqlx::query_as::<_, T>(&query)
            .fetch_all(&self.conn) // No mut
            .await?;

        Ok(rows)
    }
}
