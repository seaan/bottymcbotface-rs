use log::error;

use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use tokio::sync::Mutex;

// Structure to hold game suggestions
#[derive(Debug, Clone)]
pub struct GameSuggestion {
    pub name: String,
    pub emoji: String,
    pub suggester: serenity::UserId,
}

// In-memory store for suggestions (replace with persistent storage if needed)
lazy_static::lazy_static! {
    static ref GAME_SUGGESTIONS: Mutex<Vec<GameSuggestion>> = Mutex::new(Vec::new());
}

/// Gamenight command group
#[poise::command(slash_command, subcommands("suggest", "vote"))]
pub async fn gamenight(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
/// Create a poll from current game suggestions and clear the store
#[poise::command(slash_command)]
pub async fn vote(
    ctx: Context<'_>,
    #[description = "When will the game night occur?"] time: String,
) -> Result<(), Error> {
    // Lock, clone, and clear suggestions before any await
    let suggestions: Vec<GameSuggestion> = {
        let mut guard = GAME_SUGGESTIONS.lock().await;
        if guard.is_empty() {
            // Drop lock before await
            drop(guard);
            ctx.reply("No game suggestions to vote on!").await?;
            return Ok(());
        }
        let suggestions = guard.clone();
        guard.clear();
        suggestions
    };

    // Build embed mapping emoji to game
    let mut description = String::new();
    for s in &suggestions {
        let suggester_name = match s.suggester.to_user(ctx.serenity_context()).await {
            Ok(user) => user.name,
            Err(_) => format!("<@{}>", s.suggester),
        };
        description.push_str(&format!(
            "{} — {} (by {})\n",
            s.emoji, s.name, suggester_name
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("Gamenight Vote! Time: {}", time))
        .description(description)
        .color(serenity::Colour::BLITZ_BLUE);

    let msg_result = ctx
        .send(poise::CreateReply {
            content: Some("React below to vote for your game!".to_string()),
            embeds: vec![embed],
            ..Default::default()
        })
        .await;

    if let Ok(msg_cow) = msg_result {
        let sent_msg = msg_cow.into_message().await?;
        for s in &suggestions {
            let emoji = &s.emoji;
            let reaction_type = if emoji.starts_with('<') && emoji.ends_with('>') {
                // Custom emoji: <a:name:id> or <:name:id>
                // Extract id and name
                let parts: Vec<&str> = emoji.trim_matches(&['<', '>'][..]).split(':').collect();
                if parts.len() == 3 {
                    let animated = parts[0] == "a";
                    let name = parts[1].to_string();
                    if let Ok(id) = parts[2].parse::<u64>() {
                        serenity::ReactionType::Custom {
                            animated,
                            id: serenity::EmojiId::new(id),
                            name: Some(name),
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            } else if emoji.chars().count() == 1 {
                // Single Unicode emoji
                serenity::ReactionType::Unicode(emoji.clone())
            } else {
                // Not a valid emoji for reaction
                error!("Invalid emoji format: {}", emoji);
                continue;
            };
            let _ = sent_msg.react(ctx.serenity_context(), reaction_type).await;
        }
    }
    Ok(())
}

/// Suggest a game for gamenight
#[poise::command(slash_command)]
pub async fn suggest(
    ctx: Context<'_>,
    #[description = "Name of the game"] name: String,
    #[description = "Emoji for voting"] emoji: String,
) -> Result<(), Error> {
    let suggestion = GameSuggestion {
        name: name.clone(),
        emoji: emoji.clone(),
        suggester: ctx.author().id,
    };
    {
        let mut suggestions = GAME_SUGGESTIONS.lock().await;
        suggestions.push(suggestion);
    }
    ctx.reply(format!(
        "Game '{}' with emoji '{}' has been suggested!",
        name, emoji
    ))
    .await?;
    Ok(())
}
