use crate::{Context, Error};
use serenity::model::prelude::*;

// Structure to hold game suggestions
#[derive(Debug, Clone)]
pub struct GameSuggestion {
    pub name: String,
    pub emoji: String,
    pub suggester: UserId,
}

// In-memory store for suggestions (replace with persistent storage if needed)
lazy_static::lazy_static! {
    static ref GAME_SUGGESTIONS: std::sync::Mutex<Vec<GameSuggestion>> = std::sync::Mutex::new(Vec::new());
}

/// Gamenight command group
#[poise::command(slash_command, subcommands("suggest"))]
pub async fn gamenight(_ctx: Context<'_>) -> Result<(), Error> {
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
        let mut suggestions = GAME_SUGGESTIONS.lock().unwrap();
        suggestions.push(suggestion);
    }
    ctx.reply(format!("Game '{}' with emoji '{}' has been suggested!", name, emoji)).await?;
    Ok(())
}
