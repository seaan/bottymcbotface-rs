use std::fmt;
use std::sync::Arc;

use crate::data::db;

use sqlx::{FromRow, Pool, Sqlite};
use tokio::sync::Mutex;

use super::db::BotDatabase;

#[derive(FromRow, Debug, Clone, PartialEq, Eq)]
pub struct FeatureRequest {
    pub id: i32,
    pub request: String,
    pub user: String,
    pub votes: i32,
}

pub struct Requests {
    db: Arc<Mutex<BotDatabase>>,
}

impl fmt::Display for FeatureRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}. {} *[<@{}>] (votes: {})*",
            self.id, self.request, self.user, self.votes
        )
    }
}

impl Requests {
    pub fn new(db: Arc<Mutex<db::BotDatabase>>) -> Requests {
        Requests { db }
    }

    async fn init(&self, conn: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request TEXT NOT NULL,
                user TEXT NOT NULL,
                votes INTEGER DEFAULT 0
            )
        "#;
        sqlx::query(query).execute(conn).await?;

        Ok(())
    }

    /// Return a vector of all active requests.
    pub async fn get_requests(&self) -> Result<Vec<FeatureRequest>, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        let requests: Vec<FeatureRequest> =
            sqlx::query_as("SELECT * FROM requests ORDER BY votes DESC LIMIT 20")
                .fetch_all(conn)
                .await?;

        Ok(requests)
    }

    /// Add a new request to the database
    pub async fn add_request(
        &self,
        req: String,
        user: String,
    ) -> Result<FeatureRequest, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        self.init(conn).await?;

        let query = "INSERT INTO requests (request, user, votes) VALUES (?, ?, 0)";
        sqlx::query(query)
            .bind(req.clone())
            .bind(user.clone())
            .execute(conn)
            .await?;

        // readback the stored req
        let query = "SELECT * FROM requests ORDER BY id DESC LIMIT 1";
        let req = sqlx::query_as::<_, FeatureRequest>(query)
            .fetch_one(conn)
            .await?;

        Ok(req)
    }

    /// Mark a request as complete by deleting it from the database
    pub async fn complete_request(&self, id: i32) -> Result<FeatureRequest, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        let read_req =
            sqlx::query_as::<_, FeatureRequest>("SELECT * FROM requests WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_one(conn)
                .await?;

        let query = format!("DELETE FROM requests WHERE id = {}", id);
        sqlx::query(&query).execute(conn).await?;

        Ok(read_req)
    }

    /// Vote for a request
    pub async fn vote_request(&self, id: i32) -> Result<FeatureRequest, sqlx::Error> {
        let db_lock = self.db.lock().await;
        let conn = db_lock.get_conn();

        let query = "UPDATE requests SET votes = votes + 1 WHERE id = ?";
        sqlx::query(query).bind(id).execute(conn).await?;

        let query = "SELECT * FROM requests WHERE id = ? LIMIT 1";
        let req = sqlx::query_as::<_, FeatureRequest>(query)
            .bind(id)
            .fetch_one(conn)
            .await?;

        Ok(req)
    }
}
