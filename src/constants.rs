use log::info;
use std::env::var;

pub const QUOTES_CHANNEL_ID: u64 = 630235116475514891; // #quotes
pub const DEV_DM_CHANNEL_ID: u64 = 563105728341082148; // #dm to sean

pub fn get_update_channel_id() -> u64 {
    match var("BOT_ENV") {
        Ok(env) if env == "production" => {
            info!("Sending updates to production channel");
            QUOTES_CHANNEL_ID
        }
        _ => {
            // Default to development/testing database URL
            info!("Sending updates to development channel");
            DEV_DM_CHANNEL_ID
        }
    }
}
