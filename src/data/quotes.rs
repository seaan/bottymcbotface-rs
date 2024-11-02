use std::sync::Arc;

use crate::data::db;

use poise::serenity_prelude as serenity;
use sqlx::FromRow;
use tokio::sync::Mutex;

use super::db::BotDatabase;

#[derive(FromRow, Debug, Clone)]
pub struct QuoteMessage {
    pub id: i32,
    pub quote: String,
    pub author: String,
}

impl QuoteMessage {
    /// Create an embed for this message.
    pub fn create_embed(&self) -> serenity::CreateEmbed {
        // Initialize the embed with the title and timestamp
        serenity::CreateEmbed::default()
            .title(format!("Quote by {}", self.author))
            .color(serenity::Colour::GOLD)
            .description(format!("## {}", self.quote))
            .footer(serenity::CreateEmbedFooter::new(format!("{}", self.id)))
    }
}

pub struct Quotes {
    db: Arc<Mutex<BotDatabase>>,
}

impl Quotes {
    pub fn new(db: Arc<Mutex<db::BotDatabase>>) -> Quotes {
        Quotes { db }
    }

    /// Return a random quote from the db.
    pub async fn get_random_quote(
        &self,
        author: Option<String>,
    ) -> Result<QuoteMessage, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        let query = match author {
            Some(author) => format!(
                "SELECT * FROM quotes WHERE author = '{}' ORDER BY RANDOM() LIMIT 1",
                author
            ),
            None => "SELECT * FROM quotes ORDER BY RANDOM() LIMIT 1".to_string(),
        };

        let quote = sqlx::query_as::<_, QuoteMessage>(&query)
            .fetch_one(conn)
            .await?;

        Ok(quote)
    }

    pub async fn add_quote(
        &self,
        quote: String,
        author: String,
    ) -> Result<QuoteMessage, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        let query = "INSERT INTO quotes (quote, author) VALUES (?, ?)";
        sqlx::query(query)
            .bind(quote.clone())
            .bind(author.clone())
            .execute(conn)
            .await?;

        // readback the stored quote
        let query = "SELECT * FROM quotes ORDER BY id DESC LIMIT 1";
        let quote = sqlx::query_as::<_, QuoteMessage>(query)
            .fetch_one(conn)
            .await?;

        Ok(quote)
    }
}
