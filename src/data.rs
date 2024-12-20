pub mod bestof;
pub mod db;
pub mod quotes;
pub mod requests;

use crate::events::mentionme::RobotQuotes;

use std::sync::Arc;
use tokio::sync::Mutex;

// Custom user data passed to all command functions
pub struct Data {
    pub db: Arc<Mutex<db::BotDatabase>>,
    pub quotes_for_response: Mutex<RobotQuotes>,
    pub bestof: Arc<Mutex<bestof::BestOf>>,
    pub quotes: Arc<Mutex<quotes::Quotes>>,
    pub requests: Arc<Mutex<requests::Requests>>,
}

impl Data {
    pub fn new() -> Data {
        let db = Arc::new(Mutex::new(db::BotDatabase::new()));
        Data {
            db: db.clone(),
            quotes_for_response: Mutex::new(RobotQuotes::new()),
            bestof: Arc::new(Mutex::new(bestof::BestOf::new())),
            quotes: Arc::new(Mutex::new(quotes::Quotes::new(db.clone()))),
            requests: Arc::new(Mutex::new(requests::Requests::new(db))),
        }
    }
}
