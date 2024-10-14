pub mod mentionme;

use crate::Error;
use log::warn;
use mentionme::RobotQuotes;
use poise::serenity_prelude as serenity;
use tokio::sync::Mutex;

/// Central handler for new message events. Routes to a few different functionalities.
pub async fn handle_message_event(
    ctx: serenity::Context,
    msg: serenity::Message,
    quotes_for_response: &Mutex<RobotQuotes>,
) -> Result<(), Error> {
    // Check if the bot is mentioned in the message
    if msg.mentions_me(&ctx.http).await.unwrap_or(false) {
        let mut quotes_guard = quotes_for_response.lock().await;

        if let Err(why) = mentionme::handle_mention_event(ctx, msg, &mut *quotes_guard).await {
            warn!("Error responding to mention: {:?}", why);
        }
    }
    Ok(())
}
