use std::collections::{HashMap, HashSet};

use log::{debug, info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::sync::Mutex;

const MESSAGES_TO_CHECK: u8 = 10;
const MINIMUM_REACTIONS: usize = 5;

/// Trigger a recount of reactions on the last 5 days worth of messages. Store any updates.
pub async fn count_and_store_reactions(
    ctx: &serenity::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting reaction counting..");

    count_reactions_across_channels(ctx).await?;
    post_update(ctx).await?;

    Ok(())
}

pub async fn count_reactions_across_channels(
    ctx: &serenity::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let thicc_guild = serenity::GuildId::new(561602796286378029);

    let reacted_messages_per_channel = Arc::new(Mutex::new(HashMap::new()));
    let mut denylist = HashSet::new();
    denylist.insert(serenity::ChannelId::new(742849340418293913));
    denylist.insert(serenity::ChannelId::new(563103686612746250));
    denylist.insert(serenity::ChannelId::new(563220076707315754));
    denylist.insert(serenity::ChannelId::new(871756996452438086));
    denylist.insert(serenity::ChannelId::new(748004690418991117));

    // Collect all tasks for each channel into a vector of futures
    let tasks: Vec<_> = thicc_guild
        .channels(&ctx.http)
        .await?
        .into_keys()
        .filter(|channel_id| !denylist.contains(channel_id)) // Skip channels in denylist
        .map(|channel_id| {
            let ctx = ctx.clone();
            let reacted_messages_per_channel = Arc::clone(&reacted_messages_per_channel);
            tokio::spawn(async move {
                match parse_reactions_from_channel(&ctx, channel_id).await {
                    Err(why) => {
                        warn!("Failed to search channel {channel_id}: {:#?}", why);
                    }
                    Ok(reactions) => {
                        if let Some(reacted_messages) = reactions {
                            if reacted_messages.len() > 0 {
                                info!("Adding {:?} messages from {channel_id} to reacted_messages_per_channel", reacted_messages.len());
                                reacted_messages_per_channel
                                    .lock()
                                    .await
                                    .insert(channel_id, reacted_messages);
                                info!("Done adding from {channel_id}"); 
                            }
                        }
                    }
                }
            })
        })
        .collect();

    info!("Dispatched all reaction counting tasks");
    // Await the completion of all tasks concurrently
    futures::future::join_all(tasks).await;
    info!("Successfully joined all tasks");

    let list_of_messages = reacted_messages_per_channel.lock().await;
    if list_of_messages.len() > 0 {
        info!("Found reacted messages: {:#?}", list_of_messages);
    } else {
        info!("Couldn't find any reacted messages");
    }

    Ok(())
}

pub async fn parse_reactions_from_channel(
    ctx: &serenity::Context,
    channel_id: serenity::ChannelId,
) -> Result<Option<Vec<serenity::Message>>, Box<dyn std::error::Error + Send + Sync>> {
    match channel_id.to_channel(&ctx.http).await?.guild() {
        None => return Ok(None), // not a guild channel, just pass
        Some(channel) => Ok(Some(trawl_messages_for_reactions(ctx, channel).await?)),
    }
}

pub async fn trawl_messages_for_reactions(
    ctx: &serenity::Context,
    channel: serenity::GuildChannel,
) -> Result<Vec<serenity::Message>, Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "Channel ID: {:?}, Channel Name: {:?}",
        channel.id, channel.name
    );
    let builder = serenity::GetMessages::new().limit(MESSAGES_TO_CHECK);

    match channel.id.messages(&ctx.http, builder).await {
        Ok(mut retrieved_messages) => {
            return Ok(get_reacted_messages(&mut retrieved_messages).await);
        }
        Err(why) => {
            warn!("Failed to retrieve messages: {:#?}", why);
            return Err(Box::new(why));
        }
    }
}

async fn get_reacted_messages(
    retrieved_messages: &mut Vec<serenity::Message>,
) -> Vec<serenity::Message> {
    let mut reacted_messages: Vec<serenity::Message> = Vec::new();
    for message in retrieved_messages.drain(..) {
        match message_meets_criteria(message).await {
            None => continue,
            Some(message) => reacted_messages.push(message),
        }
    }
    reacted_messages
}

pub async fn message_meets_criteria(message: serenity::Message) -> Option<serenity::Message> {
    if message.reactions.len() < MINIMUM_REACTIONS {
        return None;
    }

    debug!(
        "Found message with {MINIMUM_REACTIONS} or more reactions: {:#?}",
        message
    );

    return Some(message);
}

pub async fn post_update(ctx: &serenity::Context) -> Result<(), Box<dyn std::error::Error>> {
    // Handle sending the message and propagate any error that occurs
    // let update_channel = serenity::ChannelId::new(563105728341082148); // DM for now
    // update_channel
    //     .send_message(&ctx.http, serenity::CreateMessage::new().content("Hello!"))
    //     .await?;

    info!("Done searching");

    Ok(())
}
