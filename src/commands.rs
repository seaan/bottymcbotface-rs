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

/// Respond with an orange
#[poise::command(slash_command, track_edits)]
pub async fn orange(
    ctx: Context<'_>,
    #[description = "Post an orange."]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    _cmd: Option<String>,
) -> Result<(), Error> {
    ctx.say("🍊").await?;
    Ok(())
}

#[poise::command(slash_command, track_edits, subcommands("random"))]
pub async fn bestof(
    ctx: Context<'_>,
    #[description = "Post a random bestof."]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    _cmd: Option<String>,
) -> Result<(), Error>{
    ctx.data().bestof.lock().await.post_random(ctx.serenity_context(), ctx.channel_id()).await
}

#[poise::command(slash_command, track_edits)]
pub async fn random(
    ctx: Context<'_>,
    #[description = "Post a random bestof."]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    _cmd: Option<String>,
)  -> Result<(), Error>{
    ctx.data().bestof.lock().await.post_random(ctx.serenity_context(), ctx.channel_id()).await
}
