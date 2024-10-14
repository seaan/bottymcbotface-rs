use std::fs;

use crate::Error;
use log::info;
use poise::serenity_prelude as serenity;

use rand::seq::SliceRandom;
use rand::thread_rng;

pub async fn handle_mention_event(
    ctx: serenity::Context,
    msg: serenity::Message,
    quotes_for_response: &mut RobotQuotes,
) -> Result<(), Error> {
    info!("Responding to direct mention with quote");

    match quotes_for_response.get_quote().await? {
        Some(quote) => {
            msg.channel_id.say(&ctx.http, quote).await?;
        }
        None => {
            msg.channel_id.say(&ctx.http, "Hello there!").await?;
        }
    }
    Ok(())
}

pub struct RobotQuotes {
    lines: Option<Vec<String>>,
}

impl RobotQuotes {
    pub fn new() -> RobotQuotes {
        RobotQuotes { lines: None }
    }

    async fn ensure_loaded(&mut self) -> Result<(), Error> {
        if self.lines.is_none() {
            // Load file contents into the `lines` field
            let raw_contents = fs::read_to_string("./data/k2so.txt")?;
            self.lines = Some(raw_contents.lines().map(|line| line.to_string()).collect());
        }
        Ok(())
    }

    pub async fn get_quote(&mut self) -> Result<Option<&str>, Error> {
        self.ensure_loaded().await?;

        let mut rng = thread_rng();
        if let Some(lines) = &self.lines {
            // Choose a random line if the file is loaded
            Ok(lines.choose(&mut rng).map(|s| s.as_str()))
        } else {
            Ok(None)
        }
    }
}
