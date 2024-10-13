use crate::{Context, Error};

/// Show this help menu
#[poise::command(prefix_command, track_edits, slash_command)]
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
