use std::time::Duration;

use anyhow::{Context as _, Error};
use futures::StreamExt;
use poise::serenity_prelude::*;

use crate::{data, PoiseContext};

#[poise::command(slash_command)]
/// タスク通知を送るチャンネルを設定します。
pub async fn set_ping_channel(ctx: PoiseContext<'_>) -> Result<(), Error> {
    ctx.data()
        .ping_channel
        .lock()
        .unwrap()
        .replace(ctx.channel_id());
    data::save(ctx.data())?;

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
    const ROLE: &str = "role";
    const SUBMIT: &str = "submit";

    let components = |role: Option<RoleId>| {
        vec![
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(
                    ROLE,
                    CreateSelectMenuKind::Role {
                        default_roles: role.map(|r| vec![r]),
                    },
                )
                .placeholder("ロールを選択してください"),
            ),
            CreateActionRow::Buttons(vec![CreateButton::new("submit")
                .custom_id(SUBMIT)
                .label("送信")
                .disabled(role.is_none())]),
        ]
    };

    let mut select = None;

    let message = ctx
        .send(
            poise::CreateReply::default()
                .embed(
                    CreateEmbed::default()
                        .title("ロールを設定してください")
                        .color(Color::DARK_BLUE),
                )
                .components(components(select)),
        )
        .await?;

    let mut interaction_stream = message
        .clone()
        .into_message()
        .await?
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(60 * 30))
        .stream();

    let mut last_interaction = None;
    while let Some(interaction) = interaction_stream.next().await {
        match &interaction.data.kind {
            ComponentInteractionDataKind::RoleSelect { values } => {
                if interaction.data.custom_id == ROLE {
                    select.replace(values[0]);
                }
                let response = CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::default().components(components(select)),
                );
                interaction.create_response(ctx, response).await?;
            }
            ComponentInteractionDataKind::Button => {
                if interaction.data.custom_id == SUBMIT {
                    last_interaction.replace(interaction);
                    break;
                }
            }
            _ => {}
        }
    }
    ctx.data()
        .ping_role
        .lock()
        .unwrap()
        .replace(select.context("No role selected")?);
    data::save(ctx.data())?;

    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::default()
            .embed(
                CreateEmbed::default()
                    .title("ロールを設定しました")
                    .description(format!("{}", select.unwrap().mention()))
                    .color(Color::DARK_BLUE),
            )
            .components(vec![]),
    );

    last_interaction
        .context("No interaction")?
        .create_response(&ctx, response)
        .await?;

    Ok(())
}
