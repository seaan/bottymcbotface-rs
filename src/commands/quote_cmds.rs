use crate::{Context, Error};

/// Messages with a certain number of reactions.
#[poise::command(slash_command, track_edits, subcommands("random"))]
pub async fn quote(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Post a random quote with an optional filter.
#[poise::command(slash_command, track_edits)]
pub async fn random(
    ctx: Context<'_>,
    #[description = "Optional author to filter by"]
    #[lazy]
    author: Option<String>,
) -> Result<(), Error> {
    // Defer the response to give more time for the command to execute
    ctx.defer().await?;

    let embed = ctx
        .data()
        .quotes
        .lock()
        .await
        .get_random_quote(author.clone())
        .await?
        .create_embed();

    let message = match author {
        Some(author) => format!("*Here's a random quote by {}:*", author),
        None => "*Here's a random quote:*".to_string(),
    };

    ctx.send(poise::CreateReply {
        content: Some(message),
        embeds: vec![embed],
        reply: true,
        ..Default::default()
    })
    .await?;

    Ok(())
}
