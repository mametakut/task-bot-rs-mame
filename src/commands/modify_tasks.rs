use std::iter;

use anyhow::{Context as _, Error};
use chrono::{Datelike, Duration, NaiveTime};
use chrono::{Local, NaiveDate, TimeZone};
use futures::StreamExt;
use itertools::Itertools;
use poise::serenity_prelude::*;
use poise::Modal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{save, Category, PoiseContext, Task};

#[poise::command(slash_command)]
/// タスクを追加します。
pub async fn add_task(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let (mut message, task) = create_task(ctx, CreateLabel::Add, None, None).await?;

    ctx.data().tasks.lock().unwrap().push(task.clone());
    save(ctx.data())?;
    message
        .edit(
            ctx,
            EditMessage::default()
                .embed(
                    CreateEmbed::default()
                        .title("タスクを追加しました")
                        .fields(vec![task.to_field()])
                        .color(Color::DARK_GREEN),
                )
                .components(vec![]),
        )
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// タスクを削除します。
pub async fn remove_task(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let (last_interaction, task) = select_task(ctx, SelectLabel::Remove).await?;

    {
        let mut tasks = ctx.data().tasks.lock().unwrap();
        let pos = tasks
            .iter()
            .position(|x| *x == task)
            .context("Task not found")?;
        tasks.remove(pos);
    }
    save(ctx.data())?;

    last_interaction
        .context("No interaction")?
        .create_response(
            ctx,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::default()
                    .embed(
                        CreateEmbed::default()
                            .title("削除しました")
                            .fields(vec![task.to_field()])
                            .color(Color::DARK_RED),
                    )
                    .components(vec![]),
            ),
        )
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
/// タスクを編集します。
pub async fn edit_task(ctx: PoiseContext<'_>) -> Result<(), Error> {
    let (last_interaction, task) = select_task(ctx, SelectLabel::Edit).await?;

    let (mut message, modified_task) = create_task(
        ctx,
        CreateLabel::Edit,
        Some(task.clone()),
        Some(last_interaction.context("No interaction")?),
    )
    .await?;

    {
        let mut tasks = ctx.data().tasks.lock().unwrap();
        let pos = tasks
            .iter()
            .position(|x| *x == task)
            .context("Task not found")?;

        tasks[pos] = modified_task.clone();
    }
    save(ctx.data())?;

    message
        .edit(
            ctx,
            EditMessage::default()
                .embed(
                    CreateEmbed::default()
                        .title("タスクを編集しました")
                        .fields(vec![
                            task.to_field(),
                            ("↓".into(), "".into(), false),
                            modified_task.to_field(),
                        ])
                        .color(Color::DARK_GREEN),
                )
                .components(vec![]),
        )
        .await?;
    Ok(())
}

#[derive(Clone, Copy)]
enum CreateLabel {
    Add,
    Edit,
}

impl From<CreateLabel> for String {
    fn from(label: CreateLabel) -> Self {
        match label {
            CreateLabel::Add => "追加",
            CreateLabel::Edit => "編集",
        }
        .to_string()
    }
}

async fn create_task(
    ctx: PoiseContext<'_>,
    label: CreateLabel,
    defaults: Option<Task>,
    interaction: Option<ComponentInteraction>,
) -> Result<(Message, Task), Error> {
    const CATEGORY: &str = "category";
    const SUBJECT: &str = "subject";
    const DATE: &str = "date";
    const TIME: &str = "time";
    const SUBMIT: &str = "submit";

    let others = Uuid::new_v4().to_string();

    let subjects = ctx.data().subjects.lock().unwrap().clone();
    let suggest_times = ctx.data().suggest_times.lock().unwrap().clone();

    let category_options = CreateSelectMenuKind::String {
        options: Category::VALUES
            .iter()
            .map(|&c| {
                CreateSelectMenuOption::new(c, c)
                    .default_selection(defaults.clone().map_or(false, |x| x.category == c))
            })
            .collect(),
    };
    let subject_options = CreateSelectMenuKind::String {
        options: subjects
            .iter()
            .map(|s| {
                CreateSelectMenuOption::new(s, s)
                    .default_selection(defaults.clone().map_or(false, |x| x.subject == *s))
            })
            .collect(),
    };
    let date_options = CreateSelectMenuKind::String {
        options: [
            (0..24)
                .map(|i| {
                    let date = Local::now().date_naive() + Duration::days(i);
                    CreateSelectMenuOption::new(
                        date.format("%Y/%m/%d (%a)").to_string(),
                        serde_json::to_string(&date).unwrap(),
                    )
                })
                .collect(),
            vec![CreateSelectMenuOption::new("その他", &others)],
        ]
        .concat(),
    };
    let time_options = CreateSelectMenuKind::String {
        options: [
            suggest_times
                .iter()
                .map(|(t, l)| {
                    CreateSelectMenuOption::new(
                        format!("{} ({})", l, t.format("%H:%M")),
                        serde_json::to_string(t).unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![CreateSelectMenuOption::new("その他", &others)],
        ]
        .concat(),
    };

    let message = {
        let embed = CreateEmbed::default()
            .title(format!("タスクを{}します", String::from(label)))
            .color(Color::DARK_BLUE);
        let components = vec![
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(CATEGORY, category_options).placeholder("カテゴリー"),
            ),
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(SUBJECT, subject_options).placeholder("教科"),
            ),
            CreateActionRow::SelectMenu(CreateSelectMenu::new(DATE, date_options).placeholder(
                defaults.clone().map_or("日付".into(), |x| {
                    x.datetime.format("%Y/%m/%d (%a)").to_string()
                }),
            )),
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(TIME, time_options).placeholder(
                    defaults
                        .clone()
                        .map_or("時間".into(), |x| x.datetime.format("%H:%M").to_string()),
                ),
            ),
            CreateActionRow::Buttons(vec![CreateButton::new(SUBMIT)
                .style(ButtonStyle::Primary)
                .label(label)]),
        ];
        if let Some(interaction) = interaction {
            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::default()
                            .embed(embed)
                            .components(components),
                    ),
                )
                .await?;
            interaction.get_response(ctx).await?
        } else {
            ctx.send(
                poise::CreateReply::default()
                    .embed(embed)
                    .components(components),
            )
            .await?
            .into_message()
            .await?
        }
    };

    let mut interaction_stream = message
        .clone()
        .await_component_interaction(ctx)
        .timeout(Duration::seconds(60 * 30).to_std()?)
        .stream();

    let mut category: Option<Category> = defaults.clone().map(|x| x.category);
    let mut subject: Option<String> = defaults.clone().map(|x| x.subject);
    let mut date: Option<NaiveDate> = defaults.clone().map(|x| x.datetime.date_naive());
    let mut time: Option<NaiveTime> = defaults.clone().map(|x| x.datetime.time());

    let mut last_interaction = None;
    while let Some(interaction) = interaction_stream.next().await {
        match &interaction.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                match interaction.data.custom_id.as_str() {
                    CATEGORY => {
                        category.replace(values[0].clone().into());
                    }
                    SUBJECT => {
                        subject.replace(values[0].clone());
                    }
                    DATE => {
                        if values[0] == others {
                            date = None;
                        } else {
                            date.replace(serde_json::from_str(&values[0])?);
                        }
                    }
                    TIME => {
                        if values[0] == others {
                            time = None;
                        } else {
                            time.replace(serde_json::from_str(&values[0])?);
                        }
                    }
                    _ => {}
                }
                interaction
                    .create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
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

    let category = category.context("Category not selected")?;
    let subject = subject.context("Subject not selected")?;

    const YEAR: &str = "year";
    const MONTH: &str = "month";
    const DAY: &str = "day";

    #[derive(Serialize, Deserialize, Clone, Copy)]
    struct MonthHalf {
        month: u32,
        is_first_half: bool,
    }

    impl From<MonthHalf> for String {
        fn from(e: MonthHalf) -> Self {
            format!(
                "{}月{}",
                e.month,
                if e.is_first_half {
                    "前半(〜15)"
                } else {
                    "後半(16〜)"
                }
            )
        }
    }

    let date = match date {
        Some(date) => date,
        None => {
            let mut date = Local::now().date_naive();

            let components = |date: NaiveDate| -> Result<_, Error> {
                let month = date.month();
                let is_first_half = date.day() <= 15;

                let year_options = CreateSelectMenuKind::String {
                    options: (Local::now().year()..=Local::now().year() + 2)
                        .map(|i| {
                            CreateSelectMenuOption::new(i.to_string(), i.to_string())
                                .default_selection(i == date.year())
                        })
                        .collect(),
                };
                let month_options = CreateSelectMenuKind::String {
                    options: (1..=12)
                        .flat_map(|i| {
                            [
                                MonthHalf {
                                    month: i,
                                    is_first_half: true,
                                },
                                MonthHalf {
                                    month: i,
                                    is_first_half: false,
                                },
                            ]
                        })
                        .map(|e| {
                            CreateSelectMenuOption::new(
                                String::from(e),
                                serde_json::to_string(&e).unwrap(),
                            )
                            .default_selection(month == e.month && is_first_half == e.is_first_half)
                        })
                        .collect(),
                };
                let day_options = CreateSelectMenuKind::String {
                    options: if is_first_half {
                        1..=15
                    } else {
                        16..=days_in_month(date.year(), month)?
                    }
                    .map(|i| {
                        CreateSelectMenuOption::new(i.to_string(), i.to_string())
                            .default_selection(i == date.day())
                    })
                    .collect(),
                };

                Ok(vec![
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(YEAR, year_options).placeholder("年"),
                    ),
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(MONTH, month_options).placeholder("月"),
                    ),
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(DAY, day_options).placeholder("日"),
                    ),
                    CreateActionRow::Buttons(vec![CreateButton::new(SUBMIT)
                        .style(ButtonStyle::Primary)
                        .label(label)]),
                ])
            };

            let response = CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::default().components(components(date)?),
            );
            last_interaction
                .clone()
                .context("No interaction")?
                .create_response(ctx, response)
                .await?;

            fn days_in_month(year: i32, month: u32) -> Result<u32, Error> {
                // 次の月の1日から1日引くことで、その月の最終日を取得
                let next_month = if month == 12 { 1 } else { month + 1 };
                let next_year = if month == 12 { year + 1 } else { year };

                let last_day = NaiveDate::from_ymd_opt(next_year, next_month, 1)
                    .context("Invalid date")?
                    .pred_opt()
                    .context("Invalid date")?;

                Ok(last_day.day())
            }

            while let Some(interaction) = interaction_stream.next().await {
                match &interaction.data.kind {
                    ComponentInteractionDataKind::StringSelect { values } => {
                        match interaction.data.custom_id.as_str() {
                            YEAR => {
                                date = date
                                    .with_year(values[0].parse().unwrap())
                                    .context("Invalid date")?;
                                interaction
                                    .create_response(ctx, CreateInteractionResponse::Acknowledge)
                                    .await?;
                            }
                            MONTH => {
                                let selected_month: MonthHalf = serde_json::from_str(&values[0])?;
                                date = date
                                    .with_month(selected_month.month)
                                    .and_then(|d| {
                                        d.with_day(if selected_month.is_first_half {
                                            1
                                        } else {
                                            16
                                        })
                                    })
                                    .context("Invalid date")?;

                                let response = CreateInteractionResponse::UpdateMessage(
                                    CreateInteractionResponseMessage::default()
                                        .components(components(date)?),
                                );
                                interaction.create_response(ctx, response).await?;
                            }
                            DAY => {
                                date = date
                                    .with_day(values[0].parse().unwrap())
                                    .context("Invalid date")?;
                                interaction
                                    .create_response(ctx, CreateInteractionResponse::Acknowledge)
                                    .await?;
                            }
                            _ => {}
                        }
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

            date
        }
    };

    const HOUR: &str = "hour";
    const MINUTE: &str = "minute";

    let time = match time {
        Some(time) => time,
        None => {
            let hour_options = CreateSelectMenuKind::String {
                options: (0..24)
                    .map(|i| CreateSelectMenuOption::new(i.to_string(), i.to_string()))
                    .collect(),
            };
            let minute_options = CreateSelectMenuKind::String {
                options: (0..60)
                    .step_by(5)
                    .chain(iter::once(59))
                    .map(|i| CreateSelectMenuOption::new(i.to_string(), i.to_string()))
                    .collect(),
            };

            let response = CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::default().components(vec![
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(HOUR, hour_options).placeholder("時"),
                    ),
                    CreateActionRow::SelectMenu(
                        CreateSelectMenu::new(MINUTE, minute_options).placeholder("分"),
                    ),
                    CreateActionRow::Buttons(vec![CreateButton::new(SUBMIT)
                        .style(ButtonStyle::Primary)
                        .label(label)]),
                ]),
            );
            last_interaction
                .clone()
                .context("No interaction")?
                .create_response(ctx, response)
                .await?;

            let mut hour = None;
            let mut minute = None;

            while let Some(interaction) = interaction_stream.next().await {
                match &interaction.data.kind {
                    ComponentInteractionDataKind::StringSelect { values } => {
                        match interaction.data.custom_id.as_str() {
                            HOUR => {
                                hour.replace(values[0].parse().unwrap());
                                interaction
                                    .create_response(ctx, CreateInteractionResponse::Acknowledge)
                                    .await?;
                            }
                            MINUTE => {
                                minute.replace(values[0].parse().unwrap());
                                interaction
                                    .create_response(ctx, CreateInteractionResponse::Acknowledge)
                                    .await?;
                            }
                            _ => {}
                        }
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

            NaiveTime::from_hms_opt(
                hour.context("Hour not selected")?,
                minute.context("Minute not selected")?,
                0,
            )
            .context("Invalid datetime")?
        }
    };

    let datetime = Local
        .from_local_datetime(&date.and_time(time))
        .single()
        .context("Invalid datetime")?;

    #[derive(Modal)]
    #[name = "詳細入力"]
    struct DetailsModal {
        #[name = "詳細を入力してください"]
        #[placeholder = "詳細"]
        details: String,
    }

    let DetailsModal { details } = poise::execute_modal_on_component_interaction::<DetailsModal>(
        ctx,
        last_interaction.context("No interaction")?,
        defaults
            .clone()
            .map(|x| DetailsModal { details: x.details }),
        None,
    )
    .await?
    .context("No interaction")?;

    let task = Task {
        category,
        subject,
        details,
        datetime,
    };

    Ok((message, task))
}

#[derive(Clone, Copy)]
enum SelectLabel {
    Remove,
    Edit,
}

impl From<SelectLabel> for String {
    fn from(label: SelectLabel) -> Self {
        match label {
            SelectLabel::Remove => "削除",
            SelectLabel::Edit => "編集",
        }
        .to_string()
    }
}

async fn select_task(
    ctx: PoiseContext<'_>,
    label: SelectLabel,
) -> Result<(Option<ComponentInteraction>, Task), Error> {
    const TASK: &str = "task";
    const SUBMIT: &str = "submit";
    const PREV: &str = "prev";
    const NEXT: &str = "next";

    let mut page = 0;
    let components = |page: usize| {
        let options = ctx
            .data()
            .tasks
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .sorted_by_key(|(_, task)| task.datetime)
            .rev()
            .map(|(idx, task)| CreateSelectMenuOption::new(task.to_field().0, idx.to_string()))
            .skip(25 * page)
            .collect::<Vec<_>>();
        let task_options = CreateSelectMenuKind::String {
            options: options.clone().into_iter().take(25).collect(),
        };

        vec![
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(TASK, task_options).placeholder("タスク"),
            ),
            CreateActionRow::Buttons(vec![
                CreateButton::new(PREV)
                    .label("前のページ")
                    .style(ButtonStyle::Secondary)
                    .disabled(page == 0),
                CreateButton::new(NEXT)
                    .label("次のページ")
                    .style(ButtonStyle::Secondary)
                    .disabled(options.len() <= 25),
            ]),
            CreateActionRow::Buttons(vec![CreateButton::new(SUBMIT)
                .style(ButtonStyle::Primary)
                .label(label)]),
        ]
    };

    let message = ctx
        .send(
            poise::CreateReply::default()
                .embed(
                    CreateEmbed::default()
                        .title(format!("{}するタスクを選択", String::from(label)))
                        .color(Color::DARK_BLUE),
                )
                .components(components(page)),
        )
        .await?;

    let mut interaction_stream = message
        .clone()
        .into_message()
        .await?
        .await_component_interaction(ctx)
        .timeout(Duration::seconds(60 * 30).to_std()?)
        .stream();

    let mut task: Option<Task> = None;
    let mut last_interaction = None;
    while let Some(interaction) = interaction_stream.next().await {
        match &interaction.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                if interaction.data.custom_id == TASK {
                    task.replace(
                        ctx.data()
                            .tasks
                            .lock()
                            .unwrap()
                            .get(values[0].parse::<usize>().unwrap())
                            .cloned()
                            .context("Invalid task")?,
                    );
                }
                interaction
                    .create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
            ComponentInteractionDataKind::Button => match interaction.data.custom_id.as_str() {
                PREV => {
                    page = page.saturating_sub(1);
                    let response = CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::default().components(components(page)),
                    );
                    interaction.create_response(ctx, response).await?;
                }
                NEXT => {
                    page += 1;
                    let response = CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::default().components(components(page)),
                    );
                    interaction.create_response(ctx, response).await?;
                }
                SUBMIT => {
                    last_interaction.replace(interaction);
                    break;
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok((last_interaction, task.context("Task not selected")?))
}
