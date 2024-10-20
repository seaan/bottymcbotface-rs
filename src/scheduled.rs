use crate::data::bestof::BestOf;
use crate::data::db::BotDatabase;

use chrono::{Duration as ChronoDuration, Utc};
use log::{error, info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub async fn spawn_scheduled_tasks(
    ctx: serenity::Context,
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
) -> tokio::task::JoinHandle<()> {
    match load_from_database(db.clone(), bestof.clone()).await {
        Err(why) => error!("Failed to update from persistent database: {:#?}", why),
        Ok(_) => info!("Successfully pulled from persistent database!"),
    }

    // Spawn the reaction counting task
    tokio::spawn(search_new_bestof_task(ctx.clone(), bestof.clone()));

    // Spawn the persistent database update task
    tokio::spawn(persistent_database_update_task(db, bestof.clone()));

    // Spawn the daily bestof posting task
    tokio::spawn(daily_bestof_task(ctx, bestof))
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

async fn daily_bestof_task(ctx: serenity::Context, bestof: Arc<Mutex<BestOf>>) {
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
        if let Err(why) = post_daily_bestof(&ctx, &bestof).await {
            warn!("Failed to post daily bestof: {:?}", why);
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
    loop {
        if let Err(why) = update_bestof_persisted_db(&db, &bestof).await {
            warn!("Failed to persist bestof data: {:?}", why);
        }

        let sleep_duration = Duration::from_secs(3610);
        info!("Next reaction counting after {:?}", sleep_duration);
        sleep(sleep_duration).await;
    }
}

async fn post_daily_bestof(
    ctx: &serenity::Context,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let update_channel = serenity::ChannelId::new(563105728341082148); // DM for now

    let bestof_unlocked = bestof.lock().await;

    let embed = bestof_unlocked.get_random_bestof_embed().await?;
    let msg = serenity::CreateMessage::new()
        .embed(embed)
        .content(String::from("*Here's your daily bestof:*"));

    update_channel.send_message(&ctx.http, msg).await?;

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
