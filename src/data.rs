pub mod bestof;
pub mod db;

use crate::events::mentionme::RobotQuotes;

use std::sync::Arc;
use tokio::sync::Mutex;

// Custom user data passed to all command functions
pub struct Data {
    pub db: Arc<Mutex<db::BotDatabase>>,
    pub quotes_for_response: Mutex<RobotQuotes>,
    pub bestof: Arc<Mutex<bestof::BestOf>>,
}

impl Data {
    pub fn new() -> Data {
        let db = Arc::new(Mutex::new(db::BotDatabase::new()));
        Data {
            db: db.clone(),
            quotes_for_response: Mutex::new(RobotQuotes::new()),
            bestof: Arc::new(Mutex::new(bestof::BestOf::new())),
        }
    }
}
