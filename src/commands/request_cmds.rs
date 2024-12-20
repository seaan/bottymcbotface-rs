use crate::{Context, Error};
use poise::{serenity_prelude as serenity, CreateReply};

/// Bot feature request by voting.
#[poise::command(slash_command, track_edits, subcommands("add", "vote", "complete"))]
pub async fn request(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add a new request to the list
#[poise::command(slash_command, track_edits)]
pub async fn add(
    ctx: Context<'_>,
    #[description = "Feature request"] request: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let author = ctx.author().id.get().to_string();
    let request = ctx
        .data()
        .requests
        .lock()
        .await
        .add_request(request.clone(), author.clone())
        .await?;

    ctx.send(poise::CreateReply {
        content: Some(format!("*Stored request!*\n{}", request).to_string()),
        reply: true,
        ..Default::default()
    })
    .await?;

    Ok(())
}

/// Present a voting menu for all of the active requests
#[poise::command(slash_command, track_edits)]
pub async fn vote(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let requests = ctx.data().requests.lock().await.get_requests().await?;

    let reply = {
        let components = requests
            .iter()
            .map(|request| {
                serenity::CreateActionRow::Buttons(vec![serenity::CreateButton::new(format!(
                    "{}",
                    request.id
                ))
                .style(serenity::ButtonStyle::Primary)
                .label(format!("{}", request))])
            })
            .collect::<Vec<_>>();

        CreateReply::default()
            .content("Vote for your desired requests!")
            .components(components)
    };

    ctx.send(reply).await?;

    if let Some(mci) = serenity::ComponentInteractionCollector::new(ctx)
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(120))
        .await
    {
        let req_id = mci.data.clone().custom_id;
        let req = ctx
            .data()
            .requests
            .lock()
            .await
            .vote_request(req_id.parse::<i32>().unwrap())
            .await?;

        let reqs = ctx.data().requests.lock().await.get_requests().await?;
        let request_list = reqs
            .iter()
            .map(|request| format!("{}", request))
            .collect::<Vec<_>>()
            .join("\n");

        let mut msg = mci.message.clone();
        msg.edit(
            ctx,
            serenity::EditMessage::new()
                .content(format!("Voted for {}!\n\n{}", req.request, request_list))
                .components(vec![]), // Remove all buttons
        )
        .await?;

        mci.create_response(
            ctx,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .content(format!("Voted for {}!\n\n{}", req.request, request_list))
                    .components(vec![]), // Remove all buttons
            ),
        )
        .await?;
    }

    Ok(())
}

/// Mark a request as completed
#[poise::command(slash_command, track_edits, hide_in_help, owners_only)]
pub async fn complete(
    ctx: Context<'_>,
    #[description = "Request id"] id: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let deleted = ctx
        .data()
        .requests
        .lock()
        .await
        .complete_request(id.parse::<i32>().unwrap())
        .await?;

    ctx.reply(format!("Marked request {} as completed", deleted))
        .await?;
    Ok(())
}
