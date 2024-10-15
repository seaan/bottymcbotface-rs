use crate::data::reactions::count_and_store_reactions;

use log::{error, info};
use poise::serenity_prelude as serenity;
use tokio::time;

pub fn spawn_scheduled_tasks(ctx: serenity::Context) {
    tokio::spawn(async move {
        reaction_counting_task(ctx).await;
    });
}

async fn reaction_counting_task(ctx: serenity::Context) {
    loop {
        // Call the function to count reactions
        if let Err(why) = count_and_store_reactions(&ctx).await {
            error!("Error counting reactions: {:?}", why);
        }

        sleep_until_next_run().await;
    }
}

async fn sleep_until_next_run() {
    let sleep_duration = std::time::Duration::from_secs(20);
    info!("Next counting reactions after {:?}", sleep_duration);
    // Sleep until the next scheduled run
    time::sleep(sleep_duration).await;
}
