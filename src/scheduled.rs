use crate::data::bestof::BestOf;
use crate::data::db::BotDatabase;

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
        if let Err(why) = post_daily_bestof(&ctx, &bestof).await {
            warn!("Failed to post daily bestof: {:?}", why);
        }

        let sleep_duration = Duration::from_secs(86400);
        info!("Next posting after {:?}", sleep_duration);
        sleep(sleep_duration).await;
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
    let mut bestof_unlocked = bestof.lock().await;

    bestof_unlocked
        .post_random(
            ctx,
            update_channel,
            Some(String::from("*Here's your daily bestof:*")),
        )
        .await
}

async fn search_new_bestof(
    ctx: &serenity::Context,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = bestof.lock().await;

    bestof_unlocked.search_and_add_new_bestof(ctx).await?;
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
