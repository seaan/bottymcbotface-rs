use sqlx::{Pool, Sqlite};
use std::env::var;
use log::info;

pub struct BotDatabase {
    conn: Pool<Sqlite>,
}

impl BotDatabase {
    pub fn new() -> BotDatabase {
        let db_url = BotDatabase::get_database_url();
        let new_connection = Pool::connect_lazy(&db_url).unwrap_or_else(|_| panic!("Failed to connect to database {:?}", db_url));

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
                "sqlite://db/production.db".to_string()
            }
            _ => {
                // Default to development/testing database URL
                info!("Using development database");
                "sqlite://db/dev.db".to_string()
            }
        }
    }

    pub async fn run_migration(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./db/migrations").run(&self.conn).await?;
        Ok(())
    }
}
