use crate::{Context, Error};
use chrono::{DateTime, Utc};
use log::{debug, error};
use poise::serenity_prelude as serenity;

/// Messages with a certain number of reactions.
#[poise::command(
    slash_command,
    track_edits,
    subcommands("random", "search_history", "top")
)]
pub async fn bestof(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Post a random bestof.
#[poise::command(slash_command, track_edits)]
pub async fn random(ctx: Context<'_>) -> Result<(), Error> {
    // Defer the response to give more time for the command to execute
    ctx.defer().await?;

    let embed = ctx
        .data()
        .bestof
        .lock()
        .await
        .get_random_bestof_embed()
        .await?;

    ctx.send(poise::CreateReply {
        content: Some("*Here's a random bestof:*".to_string()),
        embeds: vec![embed],
        reply: true,
        ..Default::default()
    })
    .await?;

    Ok(())
}

/// Search the entire history of message and add to the runtime database as necessary.
#[poise::command(slash_command, track_edits, hide_in_help, owners_only)]
pub async fn search_history(
    ctx: Context<'_>,
    #[description = "Start date for the search (format: YYYY-MM-DD) or 0 to remove the limit"]
    since: String,
) -> Result<(), Error> {
    // Defer the response to give more time for the command to execute
    ctx.defer().await?;

    let since_date = if since == "0" {
        None
    } else {
        Some(DateTime::parse_from_rfc3339(&since)?.with_timezone(&Utc))
    };

    match ctx
        .data()
        .bestof
        .lock()
        .await
        .search_and_add_new_bestof(ctx.serenity_context(), since_date)
        .await
    {
        Ok(_) => debug!("Successfully searched and added new bestof"),
        Err(why) => {
            error!("Failed to search and add new bestof: {:?}", why);
        }
    }

    ctx.reply("All done!").await?;
    Ok(())
}

/// Get the top 10 most reacted messages with an optional filter.
#[poise::command(slash_command, track_edits)]
pub async fn top(
    ctx: Context<'_>,
    #[description = "Optional user to filter by"]
    #[lazy]
    user: Option<serenity::UserId>,
    #[description = "Optional channel to filter by"]
    #[lazy]
    channel: Option<serenity::ChannelId>,
    #[description = "Optional time filter (today, this_week, this_month, this_year)"]
    #[lazy]
    time_filter: Option<String>,
) -> Result<(), Error> {
    // Defer the response to give more time for the command to execute
    ctx.defer().await?;

    let top_messages = ctx
        .data()
        .bestof
        .lock()
        .await
        .get_top_reacted_messages(ctx.serenity_context(), user, channel, time_filter)
        .await?;

    let mut embeds = Vec::new();
    for message in top_messages {
        embeds.push(message.create_embed()?);
    }

    match embeds.len() {
        0 => {
            ctx.reply("No messages found :(").await?;
        }
        _ => {
            ctx.send(poise::CreateReply {
                content: Some("*Top messages:*".to_string()),
                embeds,
                reply: true,
                ..Default::default()
            })
            .await?;
        }
    }

    Ok(())
}
