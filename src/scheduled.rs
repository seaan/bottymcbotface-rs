use crate::data::bestof::BestOf;

use log::{error, info, warn};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::{sync::Mutex, time};

pub async fn spawn_scheduled_tasks(
    ctx: serenity::Context,
    bestof_db: Arc<Mutex<BestOf>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        reaction_counting_task(ctx, bestof_db).await;
    })
}

async fn reaction_counting_task(ctx: serenity::Context, bestof_db: Arc<Mutex<BestOf>>) {
    loop {
        {
            match bestof_db.try_lock() {
                Err(why) => warn!("Couldn't run scheduled update because of lock: {:?}", why),
                Ok(mut db) => {
                    if let Err(why) = db.update(&ctx).await {
                        error!("Error running bestof update: {:?}", why);
                    }
                }
            }
        }

        sleep_until_next_run().await;
    }
}

async fn sleep_until_next_run() {
    let sleep_duration = std::time::Duration::from_secs(600);
    info!("Next counting reactions after {:?}", sleep_duration);
    // Sleep until the next scheduled run
    time::sleep(sleep_duration).await;
}
