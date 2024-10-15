pub mod reactions;

use crate::events::mentionme::RobotQuotes;

use tokio::sync::Mutex;

// Custom user data passed to all command functions
pub struct Data {
    pub quotes_for_response: Mutex<RobotQuotes>,
}

impl Data {
    pub fn new() -> Data {
        Data {
            quotes_for_response: Mutex::new(RobotQuotes::new()),
        }
    }
}
