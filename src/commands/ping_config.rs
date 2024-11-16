use std::time::Duration;

use anyhow::{Context as _, Error};
use futures::StreamExt;
use poise::serenity_prelude::*;

use crate::{save, PoiseContext};

#[poise::command(slash_command)]
/// タスク通知を送るチャンネルを設定します。
pub async fn set_ping_channel(ctx: PoiseContext<'_>) -> Result<(), Error> {
    ctx.data()
        .ping_channel
        .lock()
        .unwrap()
        .replace(ctx.channel_id());
    save(ctx.data())?;
    ctx.send(
        poise::CreateReply::default().embed(
            CreateEmbed::default()
                .title("通知チャンネルを設定しました")
                .description(format!("{}", ctx.channel_id().mention()))
                .color(Color::DARK_BLUE),
        ),
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// タスク通知を送るロールを設定します。
pub async fn set_ping_role(ctx: PoiseContext<'_>) -> Result<(), Error> {
    const SET_PING_ROLE: &str = "set_ping_role";

    let message = ctx
        .send(
            poise::CreateReply::default()
                .embed(
                    CreateEmbed::default()
                        .title("ロールを設定してください")
                        .color(Color::DARK_BLUE),
                )
                .components(vec![
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(
                            SET_PING_ROLE,
                            CreateSelectMenuKind::Role {
                                default_roles: None,
                            },
                        )
                        .placeholder("ロールを選択してください"),
                    ),
                    CreateActionRow::Buttons(vec![CreateButton::new("submit")
                        .label("送信")
                        .style(ButtonStyle::Primary)]),
                ]),
        )
        .await?;
    let mut interaction_stream = message
        .clone()
        .into_message()
        .await?
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(60 * 30))
        .stream();

    let mut select = None;
    let mut last_interaction = None;
    while let Some(interaction) = interaction_stream.next().await {
        match &interaction.data.kind {
            ComponentInteractionDataKind::RoleSelect { values } => {
                select.replace(values[0]);
                interaction
                    .create_response(ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
            ComponentInteractionDataKind::Button => {
                last_interaction.replace(interaction.clone());
                break;
            }
            _ => {}
        }
    }
    ctx.data()
        .ping_role
        .lock()
        .unwrap()
        .replace(select.context("No role selected")?);
    save(ctx.data())?;
    last_interaction
        .context("No interaction")?
        .create_response(
            &ctx,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::default()
                    .embed(
                        CreateEmbed::default()
                            .title("ロールを設定しました")
                            .description(format!("{}", select.unwrap().mention()))
                            .color(Color::DARK_BLUE),
                    )
                    .components(vec![]),
            ),
        )
        .await?;
    Ok(())
}
