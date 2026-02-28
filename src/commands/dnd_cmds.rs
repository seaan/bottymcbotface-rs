use crate::{data::dnd::MonthKey, Context, Error};
use chrono::{Duration, Local};

#[poise::command(slash_command, subcommands("submit", "status", "results"))]
pub async fn dnd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

fn format_results_message(month: &str, rows: &[crate::data::dnd::RankedDate]) -> String {
    if rows.is_empty() {
        return format!("No availability submitted for {month} yet.");
    }

    let details = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| format!("{}) {} - {}", idx + 1, row.date, row.count))
        .collect::<Vec<_>>()
        .join("\n");

    format!("Top dates for {month}:\n{details}")
}

fn parse_month_for_command(month: &str) -> Result<MonthKey, String> {
    MonthKey::parse(month).map_err(|_| {
        "Invalid month. Use month names like `march` or short forms like `mar`.".to_string()
    })
}

#[poise::command(slash_command)]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Month name (e.g. march)"] month: String,
    #[description = "Your display name"] name: String,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get().to_string(),
        None => {
            ctx.reply("This command can only be used in a server.").await?;
            return Ok(());
        }
    };

    let month_key = match parse_month_for_command(&month) {
        Ok(m) => m,
        Err(msg) => {
            ctx.reply(msg).await?;
            return Ok(());
        }
    };

    let month_row = ctx.data().dnd.ensure_month(&guild_id, month_key).await?;
    let participant = ctx
        .data()
        .dnd
        .ensure_participant(month_row.id, &name)
        .await?;
    let code = ctx
        .data()
        .dnd
        .issue_login_code(participant.id, Duration::minutes(15))
        .await?;

    let base_url = std::env::var("DND_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
    let link = format!(
        "{}/dnd/{}",
        base_url.trim_end_matches('/'),
        month_row.access_token
    );

    ctx.send(poise::CreateReply {
        content: Some(format!(
            "Login code for **{}** in **{}**: `{}`\nOpen: {}",
            name,
            month_key.canonical_name(),
            code,
            link
        )),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Month name (e.g. march)"] month: String,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get().to_string(),
        None => {
            ctx.reply("This command can only be used in a server.").await?;
            return Ok(());
        }
    };

    let month_key = match parse_month_for_command(&month) {
        Ok(m) => m,
        Err(msg) => {
            ctx.reply(msg).await?;
            return Ok(());
        }
    };

    let status = ctx.data().dnd.month_status(&guild_id, month_key).await?;
    let readonly = crate::data::dnd::is_read_only(month_key, Local::now().date_naive());
    let mode = if readonly { "read-only" } else { "open" };

    ctx.reply(format!(
        "Status for **{}** ({mode}):\nParticipants: {}\nSelected days: {}",
        month_key.canonical_name(),
        status.participants,
        status.availability_rows
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn results(
    ctx: Context<'_>,
    #[description = "Month name (e.g. march)"] month: String,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get().to_string(),
        None => {
            ctx.reply("This command can only be used in a server.").await?;
            return Ok(());
        }
    };

    let month_key = match parse_month_for_command(&month) {
        Ok(m) => m,
        Err(msg) => {
            ctx.reply(msg).await?;
            return Ok(());
        }
    };

    let rows = ctx
        .data()
        .dnd
        .top_dates_for_month(&guild_id, month_key, 2)
        .await?;

    ctx.reply(format_results_message(month_key.canonical_name(), &rows))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::data::dnd::RankedDate;

    #[test]
    fn formats_results_message_with_two_rows() {
        let message = super::format_results_message(
            "march",
            &[
                RankedDate {
                    date: "2026-03-12".to_string(),
                    count: 4,
                },
                RankedDate {
                    date: "2026-03-19".to_string(),
                    count: 3,
                },
            ],
        );

        assert!(message.contains("1) 2026-03-12 - 4"));
        assert!(message.contains("2) 2026-03-19 - 3"));
    }
}
