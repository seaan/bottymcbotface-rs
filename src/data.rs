pub mod bestof;

use crate::events::mentionme::RobotQuotes;

use std::sync::Arc;
use tokio::sync::Mutex;

// Custom user data passed to all command functions
pub struct Data {
    pub quotes_for_response: Mutex<RobotQuotes>,
    pub bestof: Arc<Mutex<bestof::BestOf>>,
}

impl Data {
    pub fn new() -> Data {
        Data {
            quotes_for_response: Mutex::new(RobotQuotes::new()),
            bestof: Arc::new(Mutex::new(bestof::BestOf::new())),
        }
    }
}
