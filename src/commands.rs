use crate::{Context, Error};

/// Show this help menu
#[poise::command(track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::register_globally(ctx, &ctx.framework().options().commands).await?;
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
    ctx.say("🍊").await?;
    Ok(())
}

/// Messages with a certain number of reactions.
#[poise::command(slash_command, track_edits, subcommands("random"))]
pub async fn bestof(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data()
        .bestof
        .lock()
        .await
        .post_random(
            ctx.serenity_context(),
            ctx.channel_id(),
            Some(String::from("*Random bestof:*")),
        )
        .await
}

/// Post a random bestof.
#[poise::command(slash_command, track_edits)]
pub async fn random(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data()
        .bestof
        .lock()
        .await
        .post_random(
            ctx.serenity_context(),
            ctx.channel_id(),
            Some(String::from("*Random bestof:*")),
        )
        .await
}
