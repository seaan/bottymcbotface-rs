use crate::constants::get_update_channel_id;
use crate::data::bestof::BestOf;
use crate::data::db::BotDatabase;
use crate::data::quotes::Quotes;

use chrono::{Duration as ChronoDuration, Utc};
use log::{error, info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

pub async fn spawn_scheduled_tasks(
    ctx: serenity::Context,
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
    quotes: Arc<Mutex<Quotes>>,
) {
    match load_from_database(db.clone(), bestof.clone()).await {
        Err(why) => error!("Failed to update from persistent database: {:#?}", why),
        Ok(_) => info!("Successfully pulled from persistent database!"),
    }

    // Spawn the reaction counting task
    tokio::spawn(search_new_bestof_task(ctx.clone(), bestof.clone()));

    // Spawn the persistent database update task
    tokio::spawn(persistent_database_update_task(db.clone(), bestof.clone()));

    // Spawn the daily bestof posting task
    tokio::spawn(daily_bestof_task(ctx.clone(), db, bestof));

    // Spawn the daily quote posting task
    tokio::spawn(daily_quotes_task(ctx, quotes));
}

/// Spawn a task that flushes in-memory bestof state to SQLite on SIGINT/SIGTERM
/// before shutting down the Discord gateway shards cleanly.
pub fn spawn_shutdown_task(
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
    shard_manager: Arc<serenity::ShardManager>,
) {
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        info!("Shutdown signal received, persisting bestof data...");
        if let Err(why) = update_bestof_persisted_db(&db, &bestof).await {
            warn!("Failed to persist bestof data on shutdown: {:?}", why);
        } else {
            info!("Persisted bestof data before shutdown");
        }
        shard_manager.shutdown_all().await;
    });
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(why) = tokio::signal::ctrl_c().await {
            warn!("Failed to listen for Ctrl+C: {:?}", why);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(why) => {
                warn!("Failed to listen for SIGTERM: {:?}", why);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn load_from_database(
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = bestof.lock().await;
    let mut db_unlocked = db.lock().await;

    bestof_unlocked
        .load_from_persisted_db(&mut db_unlocked)
        .await
}

async fn daily_bestof_task(
    ctx: serenity::Context,
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
) {
    loop {
        // Calculate the duration until the next 15:00 UTC
        let now = Utc::now();
        let now_naive = now.naive_utc();
        let next_3pm = now.date_naive().and_hms_opt(15, 0, 0).unwrap();
        let duration_until_next_3pm = if now_naive.time() < next_3pm.time() {
            next_3pm - now_naive
        } else {
            next_3pm + ChronoDuration::days(1) - now_naive
        };

        // Sleep until the next 15:00 UTC
        let sleep_duration = duration_until_next_3pm
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(86400));
        info!(
            "Next posting at 15:00 UTC, sleeping for {:?}",
            sleep_duration
        );
        sleep(sleep_duration).await;

        // Post the daily bestof
        if let Err(why) = post_daily_bestof(&ctx, &db, &bestof).await {
            warn!("Failed to post daily bestof: {:?}", why);
        }
    }
}

async fn daily_quotes_task(ctx: serenity::Context, quotes: Arc<Mutex<Quotes>>) {
    loop {
        // Calculate the duration until the next 15:00 UTC
        let now = Utc::now();
        let now_naive = now.naive_utc();
        let next_3pm = now.date_naive().and_hms_opt(15, 0, 5).unwrap();
        let duration_until_next_3pm = if now_naive.time() < next_3pm.time() {
            next_3pm - now_naive
        } else {
            next_3pm + ChronoDuration::days(1) - now_naive
        };

        // Sleep until the next 15:00 UTC
        let sleep_duration = duration_until_next_3pm
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(86400));
        info!(
            "Next posting at 15:00 UTC, sleeping for {:?}",
            sleep_duration
        );
        sleep(sleep_duration).await;

        // Post the daily bestof
        if let Err(why) = post_daily_quote(&ctx, &quotes).await {
            warn!("Failed to post daily quote: {:?}", why);
        }
    }
}

async fn search_new_bestof_task(ctx: serenity::Context, bestof: Arc<Mutex<BestOf>>) {
    loop {
        if let Err(why) = search_new_bestof(&ctx, &bestof).await {
            warn!("Failed to update bestof runtime data: {:?}", why);
        }

        let sleep_duration = Duration::from_secs(600);
        info!("Next counting reactions after {:?}", sleep_duration);
        sleep(sleep_duration).await;
    }
}

async fn persistent_database_update_task(db: Arc<Mutex<BotDatabase>>, bestof: Arc<Mutex<BestOf>>) {
    let sleep_duration = Duration::from_secs(3610);
    loop {
        info!("Next persistent database update after {:?}", sleep_duration);
        sleep(sleep_duration).await;

        if let Err(why) = update_bestof_persisted_db(&db, &bestof).await {
            warn!("Failed to persist bestof data: {:?}", why);
        }
    }
}

async fn post_daily_bestof(
    ctx: &serenity::Context,
    db: &Arc<Mutex<BotDatabase>>,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let update_channel = serenity::ChannelId::new(get_update_channel_id());
    let now = Utc::now();
    let (picked, match_kind) = {
        let bestof_unlocked = bestof.lock().await;
        bestof_unlocked
            .pick_daily_bestof_at(now)
            .ok_or("No messages available")?
    };

    let embed = picked.create_embed()?;
    let msg = serenity::CreateMessage::new()
        .embed(embed)
        .content(String::from(match_kind.prelude()));

    update_channel.send_message(&ctx.http, msg).await?;

    let mut bestof_unlocked = bestof.lock().await;
    let db_unlocked = db.lock().await;
    if let Err(why) = bestof_unlocked
        .mark_posted_at(picked.id, now, &db_unlocked)
        .await
    {
        warn!(
            "Failed to record last_posted_at for message {}: {:?}",
            picked.id, why
        );
    }

    Ok(())
}

async fn search_new_bestof(
    ctx: &serenity::Context,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = bestof.lock().await;

    bestof_unlocked.search_and_add_new_bestof(ctx, None).await?;
    Ok(())
}

async fn update_bestof_persisted_db(
    db: &Arc<Mutex<BotDatabase>>,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = bestof.lock().await;
    let mut db_unlocked = db.lock().await;

    bestof_unlocked
        .update_persisted_db(&mut db_unlocked)
        .await?;
    Ok(())
}

async fn post_daily_quote(
    ctx: &serenity::Context,
    quote: &Arc<Mutex<Quotes>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let update_channel = serenity::ChannelId::new(get_update_channel_id());
    let quote_unlocked = quote.lock().await;

    let embed = quote_unlocked.get_random_quote(None).await?.create_embed();
    let msg = serenity::CreateMessage::new()
        .embed(embed)
        .content(String::from("*Here's your daily quote:*"));

    update_channel.send_message(&ctx.http, msg).await?;

    Ok(())
}
