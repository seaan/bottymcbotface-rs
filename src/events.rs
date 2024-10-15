pub mod mentionme;
use crate::data::Data;

use crate::Error;
use log::{debug, error};
use poise::serenity_prelude as serenity;

/// Central handler for new events. Routes to a few different functionalities
pub async fn handle_event(
    ctx: serenity::Context,
    event: serenity::FullEvent,
    data: &Data,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!(
        "Got an event in event handler: {:?}",
        event.snake_case_name()
    );

    // Match on the event type
    if let serenity::FullEvent::Message { new_message } = event {
        if let Err(why) = handle_message_event(ctx, new_message, data).await {
            error!("Failed to handle message: {:?}", why);
        }
    }
    Ok(())
}

/// Handler for new message events.
pub async fn handle_message_event(
    ctx: serenity::Context,
    msg: serenity::Message,
    data: &Data,
) -> Result<(), Error> {
    // Check if the bot is mentioned in the message
    if !msg.mentions_me(&ctx.http).await.unwrap_or(false) {
        return Ok(());
    }

    let mut quotes_guard = data.quotes_for_response.lock().await;
    mentionme::handle_mention_event(ctx, msg, &mut quotes_guard).await
}
