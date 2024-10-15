use crate::data::bestof::BestOf;

use log::{error, info};
use poise::serenity_prelude as serenity;
use tokio::time;

pub fn spawn_scheduled_tasks(ctx: serenity::Context) {
    tokio::spawn(async move {
        reaction_counting_task(ctx).await;
    });
}

async fn reaction_counting_task(ctx: serenity::Context) {
    let mut db = BestOf::new();

    loop {
        if let Err(why) = db.update(&ctx).await {
            error!("Error running bestof update: {:?}", why);
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
