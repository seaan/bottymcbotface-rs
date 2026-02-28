use crate::{Context, Error};

#[poise::command(slash_command, subcommands("submit", "status", "results"))]
pub async fn dnd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn submit(_ctx: Context<'_>, _month: String, _name: String) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn status(_ctx: Context<'_>, _month: String) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn results(_ctx: Context<'_>, _month: String) -> Result<(), Error> {
    Ok(())
}
