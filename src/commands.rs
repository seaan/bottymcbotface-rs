use crate::{Context, Error};
use log::{debug, error};

/// Show this help menu
#[poise::command(track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "Beep boop",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Orange
#[poise::command(slash_command, track_edits)]
pub async fn orange(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("🍊").await?;
    Ok(())
}

/// Messages with a certain number of reactions.
#[poise::command(slash_command, track_edits, subcommands("random"))]
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
#[poise::command(slash_command, track_edits)]
pub async fn history(ctx: Context<'_>) -> Result<(), Error> {
    match ctx
        .data()
        .bestof
        .lock()
        .await
        .search_and_add_new_bestof(ctx.serenity_context(), true)
        .await
    {
        Ok(_) => debug!("Successfully searched and added new bestof"),
        Err(why) => {
            error!("Failed to search and add new bestof: {:?}", why);
        }
    }
    Ok(())
}
