use crate::data::db::BotDatabase;
use crate::data::bestof::BestOf;

use log::{info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::sync::Mutex;
use rand::rngs::{StdRng, OsRng};
use rand::{Rng, SeedableRng};
use tokio::time::{sleep, Duration};

pub async fn spawn_scheduled_tasks(
    ctx: serenity::Context,
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
) -> tokio::task::JoinHandle<()> {
    // Spawn the reaction counting task
    tokio::spawn(reaction_counting_task(ctx.clone(), bestof.clone()));

    // Spawn the persistent database update task
    tokio::spawn(persistent_database_update_task(db, bestof.clone()));

    // Spawn the daily bestof posting task
    tokio::spawn(daily_bestof_task(ctx, bestof))
}

async fn daily_bestof_task(ctx: serenity::Context, bestof: Arc<Mutex<BestOf>>) {
    loop {
        sleep_random_time().await;

        if let Err(why) = post_daily_bestof(&ctx, &bestof).await {
            warn!("Failed to post daily bestof: {:?}", why);
        }

        // TODO: schedule this
        let sleep_duration = std::time::Duration::from_secs(86400);
        info!("Next posting after {:?}", sleep_duration);
        sleep(sleep_duration).await;
    }
}

async fn post_daily_bestof(
    ctx: &serenity::Context,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let update_channel = serenity::ChannelId::new(563105728341082148); // DM for now
    let mut bestof_unlocked = acquire_lock(bestof).map_err(|e| format!("bestof lock error: {:?}", e))?;

    bestof_unlocked.post_random(ctx, update_channel).await
}

async fn reaction_counting_task(ctx: serenity::Context, bestof: Arc<Mutex<BestOf>>) {
    loop {
        sleep_random_time().await;

        if let Err(why) = update_bestof_runtime_db(&ctx, &bestof).await {
            warn!("Failed to update bestof runtime data: {:?}", why);
        }

        let sleep_duration = std::time::Duration::from_secs(600);
        info!("Next counting reactions after {:?}", sleep_duration);
        sleep(sleep_duration).await;
    }
}

async fn update_bestof_runtime_db(
    ctx: &serenity::Context,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = acquire_lock(bestof).map_err(|e| format!("bestof lock error: {:?}", e))?;
    
    bestof_unlocked.update(ctx).await?;
    Ok(())
}

async fn persistent_database_update_task(
    db: Arc<Mutex<BotDatabase>>,
    bestof: Arc<Mutex<BestOf>>,
) {
    loop {
        sleep_random_time().await;

        if let Err(why) = update_bestof_persisted_db(&db, &bestof).await {
            warn!("Failed to persist bestof data: {:?}", why);
        }

        // Once an hour, offset from runtime db update by 10 seconds.
        let sleep_duration = std::time::Duration::from_secs(3610);
        info!("Next reaction counting after {:?}", sleep_duration);
        sleep(sleep_duration).await;
    }
}

/// Random jitter for the startup of tasks so they're not all holding
/// the database lock at the same time.
pub async fn sleep_random_time() {
    // Generate a random number between 1 and 30
    let mut rng = StdRng::from_rng(OsRng).unwrap();
    let sleep_secs = rng.gen_range(1..=30);

    // Convert to a Duration
    let sleep_duration = Duration::from_secs(sleep_secs);

    // Sleep for the random amount of time
    sleep(sleep_duration).await;
}

async fn update_bestof_persisted_db(
    db: &Arc<Mutex<BotDatabase>>,
    bestof: &Arc<Mutex<BestOf>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bestof_unlocked = acquire_lock(bestof).map_err(|e| format!("bestof lock error: {:?}", e))?;
    let mut db_unlocked = acquire_lock(db).map_err(|e| format!("db lock error: {:?}", e))?;
    
    bestof_unlocked.update_persisted_db(&mut db_unlocked).await?;
    Ok(())
}

fn acquire_lock<T>(lock: &Arc<Mutex<T>>) -> Result<tokio::sync::MutexGuard<'_, T>, String> {
    lock.try_lock().map_err(|_| "Couldn't acquire lock".to_string())
}
