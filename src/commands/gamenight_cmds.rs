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
    use serenity::model::channel::{Poll, PollAnswer, PollMedia, PollLayoutType};
    use serenity::model::timestamp::Timestamp;
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
    let mut suggestions = GAME_SUGGESTIONS.lock().unwrap();
    if suggestions.is_empty() {
        ctx.reply("No game suggestions to vote on!").await?;
        return Ok(());
    }

    // Build PollAnswers from suggestions
    let answers: Vec<PollAnswer> = suggestions.iter().map(|s| {
        PollAnswer {
            answer_text: PollMedia::from_text(format!("{} {}", s.emoji, s.name)),
            ..Default::default()
        }
    }).collect();

    let poll_title = format!("Gamenight Vote! Time: {}", time);

    // Try to parse the time string into a chrono DateTime
    let expiry = match chrono::DateTime::parse_from_rfc3339(&time)
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M"))
        .map(|dt| {
            let dt = match dt {
                chrono::LocalResult::Single(dt) => dt,
                _ => return None,
            };
            Some(Timestamp::from_unix_timestamp(dt.timestamp() as u64))
        }) {
        Ok(Some(ts)) => Some(ts),
        _ => None,
    };

    let poll = Poll {
        question: PollMedia::from_text(poll_title),
        answers,
        expiry,
        allow_multiselect: true,
        layout_type: PollLayoutType::Default,
        results: None,
    };

    ctx.send(|b| b.content("").poll(poll)).await?;
    suggestions.clear();
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
