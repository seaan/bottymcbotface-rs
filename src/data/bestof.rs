use std::collections::{HashMap, HashSet};

use log::{debug, error, info, warn};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ChannelId, Context, Message, MessageId};
use std::sync::Arc;
use tokio::sync::Mutex;

const MESSAGES_TO_CHECK: u8 = 50;
const MINIMUM_REACTIONS: usize = 5;

pub struct BestOf {
    runtime_db: HashMap<MessageId, Message>,
}

impl BestOf {
    pub fn new() -> BestOf {
        BestOf {
            runtime_db: HashMap::new(),
        }
    }

    /// Trigger a recount of reactions on the last 5 days worth of messages.
    /// Store any updates. Post an update on new messages to the channel.
    pub async fn update(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting reaction counting..");

        let mut current_messages = count_current_reactions_across_channels(ctx).await?;
        let new_messages = self.update_runtime_db(&mut current_messages).await?;

        post_update(ctx, new_messages).await?;

        Ok(())
    }

    /// Translate a message update into the runtime database.
    async fn update_runtime_db(
        &mut self,
        current_messages: &mut HashMap<ChannelId, Vec<Message>>,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let mut new_messages = Vec::new();

        for (channel_id, mut messages) in current_messages.drain() {
            match self
                .update_messages_for_channel(channel_id, &mut messages)
                .await
            {
                Err(why) => warn!(
                    "Failed to update runtime db for channel {channel_id}: {:#?}",
                    why
                ),
                Ok(mut new_messages_for_channel) => {
                    debug!("Found new messages {:#?}", new_messages_for_channel);
                    new_messages.append(&mut new_messages_for_channel)
                }
            }
        }

        info!("Updated runtime db size {:?}", self.runtime_db.len());
        debug!("Updated runtime db {:#?}", self.runtime_db);

        Ok(new_messages)
    }

    /// Updates the runtime database and returns a vector of any freshly added messages.
    async fn update_messages_for_channel(
        &mut self,
        _: ChannelId,
        messages: &mut Vec<Message>,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let mut new_messages_for_channel = Vec::new();

        for msg in messages.drain(..) {
            let key = msg.id;
            let value = msg.clone();

            match self.runtime_db.insert(key, value) {
                // If this is a new insertion, `insert` will return None
                None => {
                    debug!("Added new message id {:?}", key);
                    new_messages_for_channel.push(msg);
                }
                Some(_) => debug!("Updated already present message {:?}", key),
            }
        }

        Ok(new_messages_for_channel)
    }
}

async fn count_current_reactions_across_channels(
    ctx: &Context,
) -> Result<HashMap<ChannelId, Vec<Message>>, Box<dyn std::error::Error>> {
    let thicc_guild = serenity::GuildId::new(561602796286378029);

    let reacted_messages_per_channel = Arc::new(Mutex::new(HashMap::new()));
    let mut channel_denylist = HashSet::new();
    channel_denylist.insert(ChannelId::new(742849340418293913));
    channel_denylist.insert(ChannelId::new(563103686612746250));
    channel_denylist.insert(ChannelId::new(563220076707315754));
    channel_denylist.insert(ChannelId::new(871756996452438086));
    channel_denylist.insert(ChannelId::new(748004690418991117));

    // Collect all tasks for each channel into a vector of futures
    let tasks: Vec<_> = thicc_guild
        .channels(&ctx.http)
        .await?
        .into_keys()
        .filter(|channel_id| !channel_denylist.contains(channel_id)) // Skip channels in denylist
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
                                debug!("Adding {:?} messages from {channel_id} to reacted_messages_per_channel", reacted_messages.len());
                                reacted_messages_per_channel
                                    .lock()
                                    .await
                                    .insert(channel_id, reacted_messages);
                            }
                        }
                    }
                }
            })
        })
        .collect();

    // Await the completion of all tasks concurrently
    futures::future::join_all(tasks).await;

    let list_of_messages = reacted_messages_per_channel.lock().await;
    if list_of_messages.len() > 0 {
        debug!("Found reacted messages: {:#?}", list_of_messages);
    } else {
        info!("Couldn't find any reacted messages");
    }

    Ok(list_of_messages.clone())
}

async fn parse_reactions_from_channel(
    ctx: &Context,
    channel_id: ChannelId,
) -> Result<Option<Vec<Message>>, Box<dyn std::error::Error + Send + Sync>> {
    match channel_id.to_channel(&ctx.http).await?.guild() {
        None => return Ok(None), // not a guild channel, just pass
        Some(channel) => Ok(Some(trawl_messages_for_reactions(ctx, channel).await?)),
    }
}

async fn trawl_messages_for_reactions(
    ctx: &Context,
    channel: serenity::GuildChannel,
) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
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

async fn get_reacted_messages(retrieved_messages: &mut Vec<Message>) -> Vec<Message> {
    let mut reacted_messages: Vec<Message> = Vec::new();
    for message in retrieved_messages.drain(..) {
        match message_meets_criteria(message).await {
            None => continue,
            Some(message) => reacted_messages.push(message),
        }
    }
    reacted_messages
}

async fn message_meets_criteria(message: Message) -> Option<Message> {
    if message.author.bot || message.reactions.len() < MINIMUM_REACTIONS {
        return None;
    }

    debug!(
        "Found message with {MINIMUM_REACTIONS} or more reactions: {:#?}",
        message
    );

    return Some(message);
}

async fn post_update(
    ctx: &Context,
    new_messages: Vec<Message>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle sending the message and propagate any error that occurs
    let update_channel = ChannelId::new(563105728341082148); // DM for now

    for msg in new_messages {
        match create_embed(&msg) {
            Err(why) => warn!("Failed to create embed for update: {:?}", why),
            Ok(embed) => {
                match update_channel
                    .send_message(
                        &ctx.http,
                        serenity::CreateMessage::new()
                            .content("*Found and stored this bestof:*")
                            .embed(embed),
                    )
                    .await
                {
                    Err(why) => error!("Failed to send an update message: {:?}", why),
                    Ok(_) => continue,
                }
            }
        }
    }

    Ok(())
}

fn create_embed(
    message: &Message,
) -> Result<serenity::CreateEmbed, Box<dyn std::error::Error + Send>> {
    // Handle the timestamp
    let timestamp_result =
        serenity::model::Timestamp::from_unix_timestamp(message.timestamp.unix_timestamp());

    // Match on the result to handle the error appropriately
    let timestamp = match timestamp_result {
        Ok(ts) => ts,
        Err(e) => return Err(Box::new(e)), // Convert InvalidTimestamp to Box<dyn std::error::Error>
    };

    // Initialize the embed with the title and timestamp
    let mut embed = serenity::CreateEmbed::default()
        .title(format!("Message by {}", message.author.name))
        .timestamp(timestamp);

    // Set the image if there's an appropriate attachment
    if let Some(attachment) = message.attachments.first() {
        if attachment.url.ends_with(".jpg") || attachment.url.ends_with(".png") {
            embed = embed.image(attachment.url.clone());
        }
    }

    // Set the description
    embed = embed.description(format!(
        "{}\n\n-----\n[Link]({})\n*Total Number of Reactions:* {}",
        message.content,
        message.link(),
        message.reactions.len()
    ));

    Ok(embed)
}
