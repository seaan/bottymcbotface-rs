use crate::constants::get_update_channel_id;
use crate::data::db;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use futures::stream::StreamExt;
use log::{debug, info, warn};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ChannelId, Context, Message, MessageId};
use rand::RngExt;
use rand::seq::IteratorRandom;
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

const MESSAGES_TO_CHECK: u8 = 100;
const MINIMUM_REACTIONS: u64 = 5;
const MAX_RECENCY_DAYS: f64 = 365.0;
const RECENT_COOLDOWN_DAYS: f64 = 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyBestOfMatch {
    SameDay,
    SameWeek,
    Random,
}

impl DailyBestOfMatch {
    pub fn prelude(self) -> &'static str {
        match self {
            Self::SameDay => "*Here's your daily bestof (on this day!):*",
            Self::SameWeek => "*Here's your daily bestof (from this week in history!):*",
            Self::Random => "*Here's your daily bestof:*",
        }
    }
}

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
    #[sqlx(default)]
    pub last_posted_at: Option<String>,
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
            last_posted_at: None,
        })
    }

    /// Create an embed for this message.
    pub fn create_embed(&self) -> Result<serenity::CreateEmbed, Box<dyn Error + Send + Sync>> {
        // Handle the timestamp
        let timestamp_result =
            serenity::model::Timestamp::from_unix_timestamp(self.timestamp as i64);

        // Match on the result to handle the error appropriately
        let timestamp = match timestamp_result {
            Ok(ts) => ts,
            Err(e) => return Err(Box::new(e)),
        };

        // Initialize the embed with the title and timestamp
        let mut embed = serenity::CreateEmbed::default()
            .title(format!("Message by {}", self.author))
            .timestamp(timestamp)
            .url(&self.link)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "#{}",
                self.channel
            )));

        if let Some(attachment) = &self.image {
            embed = embed.image(attachment.clone());
        }

        // Set the description
        embed = embed.description(format!(
            "{}\n\n-----\n*Total Number of Reactions:* {}",
            self.content, self.count,
        ));

        Ok(embed)
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
            let channel_name = match channel_id.to_channel(ctx).await {
                Ok(serenity::Channel::Guild(channel)) => channel.name.clone(),
                Ok(serenity::Channel::Private(_)) => "Private Channel".to_string(),
                _ => "Unknown Channel".to_string(),
            };
            match self.update_messages_for_channel(ctx, &mut messages).await {
                Err(why) => warn!(
                    "Failed to update runtime db for channel '{}' (ID: {}): {:#?}",
                    channel_name, channel_id, why
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
            let mut value = match BestOfMessage::from_serenity_message(&msg, ctx).await {
                Ok(value) => value,
                Err(why) => {
                    warn!("Failed to convert message {:#?}: {:#?}", msg, why);
                    continue; // Skip this message and move on to the next
                }
            };

            let key = value.id;
            if let Some(existing) = self.runtime_db.get(&key) {
                value.last_posted_at = existing.last_posted_at.clone();
            }

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
                "INSERT INTO messages (id, author, content, link, channel, count, timestamp, image, last_posted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
             author = excluded.author,
             content = excluded.content,
             link = excluded.link,
             channel = excluded.channel,
             count = excluded.count,
             timestamp = excluded.timestamp,
             image = excluded.image,
             last_posted_at = COALESCE(excluded.last_posted_at, messages.last_posted_at)",
            )
            .bind(msg.id)
            .bind(&msg.author)
            .bind(&msg.content)
            .bind(&msg.link)
            .bind(&msg.channel)
            .bind(msg.count)
            .bind(msg.timestamp)
            .bind(&msg.image)
            .bind(&msg.last_posted_at)
            .execute(db_conn)
            .await?;
        }
        Ok(())
    }

    /// Record when a bestof message was posted in memory and persist to SQLite.
    pub async fn mark_posted_at(
        &mut self,
        id: i64,
        posted_at: DateTime<Utc>,
        persist_db: &db::BotDatabase,
    ) -> Result<(), sqlx::Error> {
        let ts = posted_at.to_rfc3339();
        if let Some(msg) = self.runtime_db.get_mut(&id) {
            msg.last_posted_at = Some(ts.clone());
        }
        sqlx::query("UPDATE messages SET last_posted_at = ? WHERE id = ?")
            .bind(ts)
            .bind(id)
            .execute(persist_db.get_conn())
            .await?;
        Ok(())
    }

    /// Pick a daily bestof message at `now`, preferring:
    /// 1. Same calendar day (`MM-DD`) from a previous year (not on recency cooldown)
    /// 2. Same calendar week (±3 days around `MM-DD`) from a previous year (not on recency cooldown)
    /// 3. Any message weighted by recency score (`days_since_posted^2`, with 7-day cooldown)
    pub fn pick_daily_bestof_at(
        &self,
        now: DateTime<Utc>,
    ) -> Option<(BestOfMessage, DailyBestOfMatch)> {
        if self.runtime_db.is_empty() {
            return None;
        }

        let today = now.date_naive();
        let mut same_day = Vec::new();
        let mut same_week = Vec::new();

        for msg in self.runtime_db.values() {
            if let Some(kind) = classify_anniversary_match(msg, today) {
                match kind {
                    DailyBestOfMatch::SameDay => same_day.push(msg.clone()),
                    DailyBestOfMatch::SameWeek => same_week.push(msg.clone()),
                    DailyBestOfMatch::Random => {}
                }
            }
        }

        if same_day
            .iter()
            .any(|m| days_since_posted(m, now) >= RECENT_COOLDOWN_DAYS)
            && let Some(picked) = pick_weighted_by_recency(same_day.clone(), now)
        {
            return Some((picked, DailyBestOfMatch::SameDay));
        }

        if same_week
            .iter()
            .any(|m| days_since_posted(m, now) >= RECENT_COOLDOWN_DAYS)
            && let Some(picked) = pick_weighted_by_recency(same_week.clone(), now)
        {
            return Some((picked, DailyBestOfMatch::SameWeek));
        }

        let all: Vec<BestOfMessage> = self.runtime_db.values().cloned().collect();
        if all
            .iter()
            .any(|m| days_since_posted(m, now) >= RECENT_COOLDOWN_DAYS)
        {
            return pick_weighted_by_recency(all, now).map(|m| (m, DailyBestOfMatch::Random));
        }

        // All messages in the DB are within the cooldown window; still prefer tier order.
        if !same_day.is_empty() {
            return pick_weighted_by_recency(same_day, now).map(|m| (m, DailyBestOfMatch::SameDay));
        }
        if !same_week.is_empty() {
            return pick_weighted_by_recency(same_week, now)
                .map(|m| (m, DailyBestOfMatch::SameWeek));
        }
        pick_weighted_by_recency(all, now).map(|m| (m, DailyBestOfMatch::Random))
    }

    /// Return an embed of a random message from the runtime db (weighted by recency).
    pub async fn get_random_bestof_embed(
        &self,
    ) -> Result<serenity::CreateEmbed, Box<dyn Error + Send + Sync>> {
        let all: Vec<BestOfMessage> = self.runtime_db.values().cloned().collect();
        match pick_weighted_by_recency(all, Utc::now()) {
            None => Err("No messages available".into()), // Handle empty runtime_db case
            Some(msg) => msg.create_embed(),
        }
    }

    /// Top 10 most reacted messages, optionally filtered.
    pub async fn get_top_reacted_messages(
        &self,
        ctx: &Context,
        user_id: Option<serenity::UserId>,
        channel_id: Option<serenity::ChannelId>,
        time_filter: Option<String>,
    ) -> Result<Vec<BestOfMessage>, Box<dyn Error + Send + Sync>> {
        let mut top_messages: Vec<BestOfMessage> = self.runtime_db.values().cloned().collect();

        // Filter by user if provided
        if let Some(user) = user_id {
            let user_name = user.to_user(&ctx.http).await?.name;
            top_messages = futures::stream::iter(top_messages)
                .filter_map(|msg| {
                    let user_name = user_name.clone();
                    async move {
                        if msg.author == user_name {
                            Some(msg)
                        } else {
                            None
                        }
                    }
                })
                .collect()
                .await;
        }

        // Filter by channel if provided
        if let Some(channel) = channel_id {
            let channel_name = match channel.to_channel(&ctx.http).await? {
                serenity::Channel::Guild(channel) => channel.name.clone(),
                serenity::Channel::Private(_) => "Private Channel".to_string(),
                _ => "Unknown Channel".to_string(),
            };
            top_messages = futures::stream::iter(top_messages)
                .filter_map(|msg| {
                    let channel_name = channel_name.clone();
                    async move {
                        if msg.channel == channel_name {
                            Some(msg)
                        } else {
                            None
                        }
                    }
                })
                .collect()
                .await;
        }

        // Filter by time if provided
        if let Some(filter) = time_filter {
            let now = Utc::now();
            let since = match filter.as_str() {
                "today" => now - Duration::days(1),
                "this_week" => now - Duration::weeks(1),
                "this_month" => now - Duration::days(30),
                "this_year" => now - Duration::days(365),
                _ => now - Duration::days(365 * 100), // Default to a very long time ago
            };

            top_messages = futures::stream::iter(top_messages)
                .filter_map(|msg| async move {
                    if msg.timestamp >= since.timestamp() as f64 {
                        Some(msg)
                    } else {
                        None
                    }
                })
                .collect()
                .await;
        }

        top_messages.sort_by_key(|b| std::cmp::Reverse(b.count));
        Ok(top_messages.into_iter().take(10).collect())
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
                        if let Some(reacted_messages) = reactions
                            && !reacted_messages.is_empty() {
                                debug!("Adding {:?} messages from {channel_id} to reacted_messages_per_channel", reacted_messages.len());
                                reacted_messages_per_channel
                                    .lock()
                                    .await
                                    .insert(channel_id, reacted_messages);
                            }
                    }
                }
            })
        })
        .collect();

    // Await the completion of all tasks concurrently
    futures::future::join_all(tasks).await;

    let list_of_messages = reacted_messages_per_channel.lock().await;
    if !list_of_messages.is_empty() {
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
#[allow(clippy::result_large_err)]
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
#[allow(clippy::result_large_err)]
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
    let update_channel = ChannelId::new(get_update_channel_id());

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
    let embed = message.create_embed()?;
    let mut msg = serenity::CreateMessage::new().embed(embed);

    if let Some(content) = prelude {
        msg = msg.content(content);
    }

    channel_to_post_to.send_message(&ctx.http, msg).await?;

    Ok(())
}

/// Takes a Message and extracts the total count of reactions.
fn total_number_of_reactions(message: &Message) -> i64 {
    let mut total: i64 = 0;

    for reaction in &message.reactions {
        total += reaction.count as i64;
    }

    total
}

pub fn days_since_posted(msg: &BestOfMessage, now: DateTime<Utc>) -> f64 {
    match msg.last_posted_at.as_deref() {
        None => MAX_RECENCY_DAYS,
        Some(ts) => match DateTime::parse_from_rfc3339(ts) {
            Ok(posted_at) => {
                let secs = (now - posted_at.with_timezone(&Utc)).num_seconds().max(0) as f64;
                (secs / 86_400.0).min(MAX_RECENCY_DAYS)
            }
            Err(_) => MAX_RECENCY_DAYS,
        },
    }
}

/// Returns a recency score in `0.0..=(MAX_RECENCY_DAYS^2)` where a higher score
/// means the message was posted less recently (or never) and is more eligible to be picked.
pub fn bestof_recency_score(msg: &BestOfMessage, now: DateTime<Utc>) -> f64 {
    let days = days_since_posted(msg, now);
    days * days
}

fn pick_weighted_by_recency(
    candidates: Vec<BestOfMessage>,
    now: DateTime<Utc>,
) -> Option<BestOfMessage> {
    if candidates.is_empty() {
        return None;
    }

    let has_cooled_down = candidates
        .iter()
        .any(|m| days_since_posted(m, now) >= RECENT_COOLDOWN_DAYS);

    let weights: Vec<f64> = candidates
        .iter()
        .map(|m| {
            let days = days_since_posted(m, now);
            if has_cooled_down && days < RECENT_COOLDOWN_DAYS {
                0.0
            } else {
                bestof_recency_score(m, now)
            }
        })
        .collect();

    let total: f64 = weights.iter().sum();
    let mut rng = rand::rng();
    if total <= 0.0 {
        return candidates.into_iter().choose(&mut rng);
    }

    let mut target = rng.random_range(0.0..total);
    let last_idx = candidates.len() - 1;
    for (idx, (m, w)) in candidates.into_iter().zip(weights).enumerate() {
        if idx == last_idx || target < w {
            return Some(m);
        }
        target -= w;
    }
    None
}

fn anniversary_date_in_year(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day).or_else(|| {
        if month == 2 && day == 29 {
            NaiveDate::from_ymd_opt(year, 2, 28)
        } else {
            None
        }
    })
}

fn classify_anniversary_match(msg: &BestOfMessage, today: NaiveDate) -> Option<DailyBestOfMatch> {
    let msg_dt = DateTime::from_timestamp(msg.timestamp as i64, 0)?;
    let msg_date = msg_dt.date_naive();

    // Only treat messages from earlier calendar years as "on this day / this week" throwbacks
    if msg_date.year() >= today.year() {
        return None;
    }

    let min_days = [today.year() - 1, today.year(), today.year() + 1]
        .into_iter()
        .filter_map(|yr| anniversary_date_in_year(yr, msg_date.month(), msg_date.day()))
        .map(|anniv| (anniv - today).num_days().abs())
        .min()?;

    if min_days == 0 {
        Some(DailyBestOfMatch::SameDay)
    } else if min_days <= 3 {
        Some(DailyBestOfMatch::SameWeek)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_msg(id: i64, y: i32, m: u32, d: u32, last_posted_at: Option<&str>) -> BestOfMessage {
        let ts = Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap().timestamp() as f64;
        BestOfMessage {
            id,
            author: "user".to_string(),
            content: format!("msg {id}"),
            link: format!("https://discord.com/channels/1/2/{id}"),
            channel: "general".to_string(),
            count: 7,
            timestamp: ts,
            image: None,
            last_posted_at: last_posted_at.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn pick_daily_bestof_prefers_same_day_anniversary() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let mut bestof = BestOf::new();
        bestof
            .runtime_db
            .insert(1, sample_msg(1, 2024, 11, 11, None));
        bestof
            .runtime_db
            .insert(2, sample_msg(2, 2024, 11, 10, None));
        bestof.runtime_db.insert(3, sample_msg(3, 2023, 5, 1, None));

        let (picked, kind) = bestof.pick_daily_bestof_at(now).unwrap();
        assert_eq!(picked.id, 1);
        assert_eq!(kind, DailyBestOfMatch::SameDay);
    }

    #[test]
    fn pick_daily_bestof_falls_back_to_same_week_when_no_same_day() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let mut bestof = BestOf::new();
        bestof
            .runtime_db
            .insert(2, sample_msg(2, 2024, 11, 9, None));
        bestof.runtime_db.insert(3, sample_msg(3, 2023, 5, 1, None));

        let (picked, kind) = bestof.pick_daily_bestof_at(now).unwrap();
        assert_eq!(picked.id, 2);
        assert_eq!(kind, DailyBestOfMatch::SameWeek);
    }

    #[test]
    fn pick_daily_bestof_falls_back_to_random_when_no_anniversary() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let mut bestof = BestOf::new();
        bestof.runtime_db.insert(3, sample_msg(3, 2023, 5, 1, None));

        let (picked, kind) = bestof.pick_daily_bestof_at(now).unwrap();
        assert_eq!(picked.id, 3);
        assert_eq!(kind, DailyBestOfMatch::Random);
    }

    #[test]
    fn pick_daily_bestof_avoids_recently_posted_same_day_in_favor_of_fresh() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let recent_iso = (now - Duration::days(2)).to_rfc3339();
        let mut bestof = BestOf::new();
        bestof
            .runtime_db
            .insert(1, sample_msg(1, 2023, 11, 11, Some(&recent_iso)));
        bestof
            .runtime_db
            .insert(2, sample_msg(2, 2024, 11, 11, None));

        for _ in 0..20 {
            let (picked, kind) = bestof.pick_daily_bestof_at(now).unwrap();
            assert_eq!(picked.id, 2);
            assert_eq!(kind, DailyBestOfMatch::SameDay);
        }
    }

    #[test]
    fn pick_daily_bestof_skips_cooled_down_same_day_tier_if_all_same_day_recent() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let recent_iso = (now - Duration::days(1)).to_rfc3339();
        let mut bestof = BestOf::new();
        bestof
            .runtime_db
            .insert(1, sample_msg(1, 2023, 11, 11, Some(&recent_iso)));
        bestof
            .runtime_db
            .insert(2, sample_msg(2, 2024, 11, 10, None));

        let (picked, kind) = bestof.pick_daily_bestof_at(now).unwrap();
        assert_eq!(picked.id, 2);
        assert_eq!(kind, DailyBestOfMatch::SameWeek);
    }

    #[test]
    fn bestof_recency_score_ranks_never_above_older_above_recent() {
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        let old_iso = (now - Duration::days(60)).to_rfc3339();
        let recent_iso = (now - Duration::days(2)).to_rfc3339();

        let never = sample_msg(1, 2023, 1, 1, None);
        let old = sample_msg(2, 2023, 1, 1, Some(&old_iso));
        let recent = sample_msg(3, 2023, 1, 1, Some(&recent_iso));

        assert!(bestof_recency_score(&never, now) > bestof_recency_score(&old, now));
        assert!(bestof_recency_score(&old, now) > bestof_recency_score(&recent, now));
    }

    #[tokio::test]
    async fn mark_posted_at_updates_runtime_and_sqlite_db() {
        let mut db = db::BotDatabase::new_with_path(":memory:");
        db.run_migration().await.unwrap();

        let mut bestof = BestOf::new();
        bestof
            .runtime_db
            .insert(42, sample_msg(42, 2024, 11, 11, None));
        bestof.update_persisted_db(&mut db).await.unwrap();

        let now = Utc.with_ymd_and_hms(2026, 11, 11, 15, 0, 0).unwrap();
        bestof.mark_posted_at(42, now, &db).await.unwrap();

        assert_eq!(
            bestof
                .runtime_db
                .get(&42)
                .unwrap()
                .last_posted_at
                .as_deref(),
            Some(now.to_rfc3339().as_str())
        );

        let mut reloaded = BestOf::new();
        reloaded.load_from_persisted_db(&mut db).await.unwrap();
        assert_eq!(
            reloaded
                .runtime_db
                .get(&42)
                .unwrap()
                .last_posted_at
                .as_deref(),
            Some(now.to_rfc3339().as_str())
        );
    }
}
