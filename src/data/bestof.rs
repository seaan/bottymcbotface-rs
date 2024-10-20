use crate::constants::QUOTES_CHANNEL_ID;
use crate::data::db;

use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ChannelId, Context, Message, MessageId};
use rand::rngs::{OsRng, StdRng};
use rand::seq::IteratorRandom;
use rand::SeedableRng;
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

const MESSAGES_TO_CHECK: u8 = 100;
const MINIMUM_REACTIONS: u64 = 5;

#[derive(FromRow, Debug, Clone)]
pub struct BestOfMessage {
    pub id: i64,
    pub author: String,
    pub content: String,
    pub link: String,
    pub channel: String,
    pub count: i64,
    pub timestamp: f64,
    pub image: Option<String>,
}

impl BestOfMessage {
    pub async fn from_serenity_message(
        message: &serenity::Message,
        ctx: &serenity::Context,
    ) -> Result<Self, Box<dyn Error>> {
        let channel_name = match message.channel_id.to_channel(ctx).await? {
            serenity::Channel::Guild(channel) => channel.name.clone(),
            serenity::Channel::Private(_) => "Private Channel".to_string(),
            _ => "Unknown Channel".to_string(),
        };

        Ok(BestOfMessage {
            id: message.id.get() as i64,                          // Message ID as i64
            author: message.author.name.clone(),                  // Author's name
            content: message.content.clone(),                     // Message content
            link: message.link(),                                 // Permalink to the message
            channel: channel_name,                                // Channel name
            count: total_number_of_reactions(message),            // Total reaction count as i64
            timestamp: message.timestamp.unix_timestamp() as f64, // Message timestamp
            image: message.attachments.first().map(|a| a.url.clone()), // Optional image URL from the attachments
        })
    }
}

pub struct BestOf {
    runtime_db: HashMap<i64, BestOfMessage>,
}

impl BestOf {
    pub fn new() -> BestOf {
        BestOf {
            runtime_db: HashMap::new(),
        }
    }

    /// Trigger a recount of reactions on the last 5 days worth of messages.
    /// Store any updates. Post an update on new messages to the channel.
    pub async fn search_and_add_new_bestof(
        &mut self,
        ctx: &Context,
        since: Option<DateTime<Utc>>,
    ) -> Result<(), Box<dyn Error>> {
        info!("Starting reaction counting..");

        let mut current_messages = count_current_reactions_across_channels(ctx, since).await?;
        let new_messages = self
            .update_runtime_db_from_new_bestof(ctx, &mut current_messages)
            .await?;

        post_update(ctx, new_messages).await?;

        Ok(())
    }

    /// Translate a message update into the runtime database.
    async fn update_runtime_db_from_new_bestof(
        &mut self,
        ctx: &Context,
        current_messages: &mut HashMap<ChannelId, Vec<Message>>,
    ) -> Result<Vec<BestOfMessage>, Box<dyn Error>> {
        let mut new_messages = Vec::new();

        for (channel_id, mut messages) in current_messages.drain() {
            match self.update_messages_for_channel(ctx, &mut messages).await {
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

        debug!("Updated runtime db size {:?}", self.runtime_db.len());
        debug!("Updated runtime db {:#?}", self.runtime_db);

        Ok(new_messages)
    }

    /// Updates the runtime database and returns a vector of any freshly added messages.
    async fn update_messages_for_channel(
        &mut self,
        ctx: &Context,
        messages: &mut Vec<Message>,
    ) -> Result<Vec<BestOfMessage>, Box<dyn Error>> {
        let mut new_messages_for_channel = Vec::new();

        for msg in messages.drain(..) {
            let value = match BestOfMessage::from_serenity_message(&msg, ctx).await {
                Ok(value) => value,
                Err(why) => {
                    warn!("Failed to convert message {:#?}: {:#?}", msg, why);
                    continue; // Skip this message and move on to the next
                }
            };

            let key = value.id;

            if self.runtime_db.insert(key, value.clone()).is_none() {
                // This is a new insertion
                debug!("Added new message id {:?}", key);
                new_messages_for_channel.push(value);
            } else {
                debug!("Updated already present message {:?}", key);
            }
        }

        Ok(new_messages_for_channel)
    }

    /// Load the runtime db from the persisted db.
    pub async fn load_from_persisted_db(
        &mut self,
        persist_db: &mut db::BotDatabase,
    ) -> Result<(), Box<dyn Error>> {
        let messages: Vec<BestOfMessage> = persist_db
            .load_all_from_table(String::from("messages"))
            .await?;

        for msg in messages {
            self.runtime_db.insert(msg.id, msg);
        }

        Ok(())
    }

    /// Update the persisted db from the runtime db.
    pub async fn update_persisted_db(
        &mut self,
        persist_db: &mut db::BotDatabase,
    ) -> Result<(), Box<dyn Error>> {
        let db_conn = persist_db.get_conn();

        // upsert all messages in runtime_db into the persisted database
        for msg in self.runtime_db.values() {
            sqlx::query(
                "INSERT INTO messages (id, author, content, link, channel, count, timestamp, image)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
             author = excluded.author,
             content = excluded.content,
             link = excluded.link,
             channel = excluded.channel,
             count = excluded.count,
             timestamp = excluded.timestamp,
             image = excluded.image",
            )
            .bind(msg.id)
            .bind(&msg.author)
            .bind(&msg.content)
            .bind(&msg.link)
            .bind(&msg.channel)
            .bind(msg.count)
            .bind(msg.timestamp)
            .bind(&msg.image)
            .execute(db_conn)
            .await?;
        }
        Ok(())
    }

    /// Return an embed of a random message from the runtime db.
    pub async fn get_random_bestof_embed(
        &self,
    ) -> Result<serenity::CreateEmbed, Box<dyn Error + Send + Sync>> {
        let mut rng = StdRng::from_rng(OsRng)?;

        match self.runtime_db.values().choose(&mut rng) {
            None => Err("No messages available".into()), // Handle empty runtime_db case
            Some(msg) => create_embed(msg),
        }
    }
}

/// Count the current reactions across all channels, with one thread
/// per channel.
async fn count_current_reactions_across_channels(
    ctx: &Context,
    since: Option<DateTime<Utc>>,
) -> Result<HashMap<ChannelId, Vec<Message>>, Box<dyn Error>> {
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
                match parse_reactions_from_channel(&ctx, channel_id, since).await {
                    Err(why) => {
                        warn!("Failed to search channel {channel_id}: {:#?}", why);
                    }
                    Ok(reactions) => {
                        if let Some(reacted_messages) = reactions {
                            if !reacted_messages.is_empty() {
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

/// Parse reactions from a channel.
async fn parse_reactions_from_channel(
    ctx: &Context,
    channel_id: ChannelId,
    since: Option<DateTime<Utc>>,
) -> Result<Option<Vec<Message>>, Box<dyn Error + Send + Sync>> {
    match channel_id.to_channel(&ctx.http).await?.guild() {
        None => Ok(None), // not a guild channel, just pass
        Some(channel) => Ok(Some(
            trawl_messages_for_reactions(ctx, channel, since).await?,
        )),
    }
}

/// Trawl through messages in a channel and return those that meet the criteria.
async fn trawl_messages_for_reactions(
    ctx: &Context,
    channel: serenity::GuildChannel,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<Message>, Box<dyn Error + Send + Sync>> {
    debug!(
        "Channel ID: {:?}, Channel Name: {:?}",
        channel.id, channel.name
    );

    match retrieve_messages(ctx, channel, since).await {
        Ok(mut retrieved_messages) => Ok(get_reacted_messages(&mut retrieved_messages).await),
        Err(why) => {
            warn!("Failed to retrieve messages: {:#?}", why);
            Err(Box::new(why))
        }
    }
}

/// Retrieve messages from a channel.
async fn retrieve_messages(
    ctx: &Context,
    channel: serenity::GuildChannel,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<Message>, serenity::Error> {
    let mut all_messages: Vec<Message> = Vec::new();
    let mut before: Option<MessageId> = None;

    loop {
        let mut messages = get_message_batch(ctx, channel.clone(), before).await?;

        if messages.is_empty() {
            break;
        }

        all_messages.append(&mut messages);

        match since {
            Some(since) => {
                before = Some(all_messages.last().unwrap().id);

                match before {
                    Some(msg_id) => {
                        if msg_id.created_at() < since.into() {
                            break;
                        }

                        info!(
                            "Found {:?} messages in channel {:?}, last message post time {:?}, continuing search..",
                            all_messages.len(),
                            channel.name,
                            msg_id.created_at().to_string()
                        );
                    }
                    None => break,
                }
            }
            None => break,
        }
    }

    Ok(all_messages)
}

/// Retrieve a batch of messages from a channel.
async fn get_message_batch(
    ctx: &Context,
    channel: serenity::GuildChannel,
    before: Option<MessageId>,
) -> Result<Vec<Message>, serenity::Error> {
    match before {
        Some(search_from) => {
            let builder = serenity::GetMessages::new()
                .limit(MESSAGES_TO_CHECK)
                .before(search_from);

            channel.id.messages(&ctx.http, builder).await
        }
        None => {
            let builder = serenity::GetMessages::new().limit(MESSAGES_TO_CHECK);

            channel.id.messages(&ctx.http, builder).await
        }
    }
}

/// Filter messages that meet the criteria.
async fn get_reacted_messages(retrieved_messages: &mut Vec<Message>) -> Vec<Message> {
    let mut reacted_messages: Vec<Message> = Vec::new();
    for message in retrieved_messages.drain(..) {
        match message_meets_criteria(message) {
            None => continue,
            Some(message) => reacted_messages.push(message),
        }
    }
    reacted_messages
}

/// Check if a message meets the criteria.
fn message_meets_criteria(message: Message) -> Option<Message> {
    if message.author.bot
        || message.reactions.is_empty()
        || number_of_users_reacted(&message) < MINIMUM_REACTIONS
    {
        return None;
    }

    debug!(
        "Found message with {MINIMUM_REACTIONS} or more on a single reaction: {:#?}",
        message
    );

    Some(message)
}

/// Takes a Message and extracts the highest count reaction.
fn number_of_users_reacted(message: &Message) -> u64 {
    let mut highest_count: u64 = 0;

    // Iterate over each reaction
    for reaction in &message.reactions {
        // Fetch the current reaction count
        let current_reaction_count = reaction.count;

        if highest_count < current_reaction_count {
            highest_count = current_reaction_count;
        }
    }

    highest_count
}

/// Post an update to the channel.
async fn post_update(
    ctx: &Context,
    new_messages: Vec<BestOfMessage>,
) -> Result<(), Box<dyn Error>> {
    let update_channel = ChannelId::new(QUOTES_CHANNEL_ID);

    for msg in new_messages {
        match post_message_as_embed(
            ctx,
            &msg,
            update_channel,
            Some(String::from("*Found and stored this bestof:*")),
        )
        .await
        {
            Err(why) => warn!("Failed to send update: {:?}", why),
            Ok(_) => continue,
        }
    }

    Ok(())
}

/// Post a message as an embed to a channel.
pub async fn post_message_as_embed(
    ctx: &Context,
    message: &BestOfMessage,
    channel_to_post_to: ChannelId,
    prelude: Option<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let embed = create_embed(message)?;
    let mut msg = serenity::CreateMessage::new().embed(embed);

    if let Some(content) = prelude {
        msg = msg.content(content);
    }

    channel_to_post_to.send_message(&ctx.http, msg).await?;

    Ok(())
}

/// Create an embed from a BestOfMessage.
fn create_embed(
    message: &BestOfMessage,
) -> Result<serenity::CreateEmbed, Box<dyn Error + Send + Sync>> {
    // Handle the timestamp
    let timestamp_result =
        serenity::model::Timestamp::from_unix_timestamp(message.timestamp as i64);

    // Match on the result to handle the error appropriately
    let timestamp = match timestamp_result {
        Ok(ts) => ts,
        Err(e) => return Err(Box::new(e)),
    };

    // Initialize the embed with the title and timestamp
    let mut embed = serenity::CreateEmbed::default()
        .title(format!("Message by {}", message.author))
        .timestamp(timestamp)
        .url(&message.link)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "#{}",
            &message.channel
        )));

    if let Some(attachment) = &message.image {
        embed = embed.image(attachment.clone());
    }

    // Set the description
    embed = embed.description(format!(
        "{}\n\n-----\n*Total Number of Reactions:* {}",
        message.content, message.count,
    ));

    Ok(embed)
}

/// Takes a Message and extracts the total count of reactions.
fn total_number_of_reactions(message: &Message) -> i64 {
    let mut total: i64 = 0;

    for reaction in &message.reactions {
        total += reaction.count as i64;
    }

    total
}
