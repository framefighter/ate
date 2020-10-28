use serde::{Deserialize, Serialize};
use teloxide::prelude::GetChatId;
use teloxide::prelude::Request;
use teloxide::types::{File as TgFile, PhotoSize};
use teloxide::types::{ReplyMarkup, User};
use teloxide::utils::command::{BotCommand, ParseError};
use tokio::fs::File;

use crate::button;
use crate::button::{poll_plan_buttons, Button, ButtonKind};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::plan::Plan;
use crate::poll::PollKind;
use crate::request::{RequestKind, RequestResult};
use crate::{ContextMessage, StateLock, VERSION};

fn create_command(
    input: String,
) -> Result<(String, Option<u8>, Option<Vec<String>>, Option<String>), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            let meal_name = name.trim().to_string();
            if meal_name.len() > 0 {
                meal_name
            } else {
                return Err(ParseError::Custom("Provide a meal name!".into()));
            }
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(rating_str) = args.get(1) {
            if let Ok(rating) = rating_str.trim().parse::<u8>() {
                Some(rating)
            } else {
                return Err(ParseError::Custom(
                    "Rating (2nd argument) has to be a number!".into(),
                ));
            }
        } else {
            None
        },
        if let Some(tags_str) = args.get(2) {
            let tags: Vec<_> = tags_str
                .trim()
                .split(" ")
                .map(|s| s.trim().to_string())
                .collect();
            Some(tags)
        } else {
            None
        },
        if let Some(url_str) = args.get(3) {
            Some(url_str.trim().to_string())
        } else {
            None
        },
    ))
}

fn meal_name_command(input: String) -> Result<(String, String), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(name) = args.get(1) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a second argument!".into()));
        },
    ))
}

fn tag_meal_command(input: String) -> Result<(String, Vec<String>), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(tags_str) = args.get(1) {
            tags_str
                .trim()
                .split(" ")
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            vec![]
        },
    ))
}

fn rate_meal_command(input: String) -> Result<(String, u8), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(rating_str) = args.get(1) {
            if let Ok(rating) = rating_str.trim().parse::<u8>() {
                rating
            } else {
                return Err(ParseError::Custom(
                    "Rating (2nd argument) has to be a number!".into(),
                ));
            }
        } else {
            return Err(ParseError::Custom("Provide a second argument!".into()));
        },
    ))
}

fn plan_command(input: String) -> Result<(Option<usize>,), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((if let Some(rating_str) = args.get(0) {
        if let Ok(rating) = rating_str.trim().parse::<usize>() {
            Some(rating)
        } else {
            None
        }
    } else {
        None
    },))
}

#[derive(BotCommand, Debug, Clone, Serialize, Deserialize)]
#[command(rename = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "List all commands.")]
    Help,
    #[command(description = "Save a meal step by step.")]
    NewMeal(String),
    #[command(description = "Save a complete meal.", parse_with = "create_command")]
    New {
        meal_name: String,
        rating: Option<u8>,
        tags: Option<Vec<String>>,
        url: Option<String>,
    },
    #[command(
        description = "Plan meals for given days.",
        parse_with = "plan_command"
    )]
    Plan(Option<usize>),
    #[command(description = "Get a saved meal's info.")]
    Get(String),
    #[command(description = "Remove a meal by name.")]
    Remove(String),
    #[command(description = "Get a list of all meals.")]
    List,
    #[command(description = "Whitelist user.", parse_with = "meal_name_command")]
    Op(String, String),
    #[command(
        description = "Rename existing meal.",
        parse_with = "meal_name_command"
    )]
    Rename(String, String),
    #[command(
        description = "Change rating of existing meal.",
        parse_with = "rate_meal_command"
    )]
    Rate(String, u8),
    #[command(
        description = "Add tags to existing meal.",
        parse_with = "tag_meal_command"
    )]
    Tag(String, Vec<String>),
    #[command(
        description = "Remove tags from existing meal.",
        parse_with = "tag_meal_command"
    )]
    TagRemove(String, Vec<String>),
    #[command(
        description = "Edit reference of existing meal.",
        parse_with = "meal_name_command"
    )]
    Ref(String, String),
    #[command(description = "Get bot version.")]
    Version,
}

impl Command {
    pub fn run(command: &Command, state: &StateLock, cx: &ContextMessage) -> RequestResult {
        let mut request = RequestResult::default();
        let user_opt = cx.update.from();
        let config = state.read().config.clone();
        match command {
            Command::Op(username, password) => {
                request.message(cx.answer(if password == &config.password {
                    state.write().whitelist_user(username.clone());
                    format!("Added user {} to whitelist.\nEnjoy!", username)
                } else {
                    format!("Wrong password: {}", password)
                }));
            }
            _ => {}
        }
        let whitelist: Vec<_> = state.read().get_whitelisted_users();
        match user_opt {
            Some(User {
                username: Some(username),
                id: user_id,
                ..
            }) => {
                if !whitelist.contains(&username.clone()) {
                    request.message(cx.answer(format!("User not whitelisted!")));
                    return request;
                } else {
                    match command {
                        Command::Op { .. } => {}
                        Command::Help => {
                            request.message(cx.answer(Command::descriptions()));
                        }
                        Command::NewMeal(meal_name) => {
                            let meal = Meal::new(meal_name, cx.chat_id(), *user_id);
                            meal.save(&state);
                            request.add(
                                meal.request(
                                    &cx,
                                    Some("How did it taste?".to_string()),
                                    Some(
                                        Keyboard::new(cx.chat_id())
                                            .buttons(vec![button::rate_meal_button_row(0, &meal)])
                                            .save(&state),
                                    ),
                                ),
                            );
                        }
                        Command::New {
                            meal_name,
                            rating,
                            tags,
                            url,
                        } => {
                            let mut meal = Meal::new(meal_name, cx.chat_id(), *user_id);
                            meal.rate(rating.clone())
                                .tag(tags.clone().unwrap_or_default())
                                .url(url.clone())
                                .save(&state);
                            request.add(
                                meal.request(
                                    &cx,
                                    None,
                                    Some(
                                        Keyboard::new(cx.chat_id())
                                            .buttons(vec![
                                                vec![Button::new(
                                                    "Rate with Poll".into(),
                                                    ButtonKind::PollRating { meal: meal.clone() },
                                                )],
                                                button::save_meal_button_row(&meal),
                                            ])
                                            .save(&state),
                                    ),
                                ),
                            );
                        }
                        Command::Get(meal_name) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            for meal in meals {
                                request.add(
                                    meal.request(
                                        &cx,
                                        None,
                                        Some(
                                            Keyboard::new(cx.chat_id())
                                                .buttons(vec![vec![Button::new(
                                                    "Cancel".to_uppercase(),
                                                    ButtonKind::DeleteMessage,
                                                )]])
                                                .save(&state),
                                        ),
                                    ),
                                );
                            }
                        }
                        Command::Remove(meal_name) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request.message(
                                    cx.answer(format!("No meal with name {} found!", meal_name)),
                                );
                            }
                            for meal in meals {
                                state.write().remove_meal(&meal);
                                request.add(meal.request(&cx, Some(format!("Deleted!")), None));
                            }
                        }
                        Command::Plan(days_opt) => {
                            let meals = state.read().get_meals(cx.chat_id());
                            let meal_count = meals.len();
                            let meal_plan = if let Some(days) = days_opt {
                                Plan::gen(cx.chat_id(), meals, *days)
                            } else {
                                state
                                    .read()
                                    .find_plan(cx.chat_id())
                                    .cloned()
                                    .unwrap_or(Plan::new(cx.chat_id(), vec![]))
                            };
                            state.write().add_plan(meal_plan.clone());
                            let keyboard = Keyboard::new(cx.chat_id())
                                .buttons(poll_plan_buttons(meal_plan.clone()))
                                .save(&state);
                            if meal_plan.days < 2 {
                                request.message(cx.bot.send_message(
                                    cx.chat_id(),
                                    format!("Plan for at least 2 days!"),
                                ));
                            } else if meal_plan.days > 10 {
                                request.message(cx.bot.send_message(
                                    cx.chat_id(),
                                    format!("Can only plan for a maximum of 10 days!"),
                                ));
                            } else if meal_plan.days > 1 {
                                request.add(RequestKind::Poll(
                                    cx.bot
                                        .send_poll(
                                            cx.chat_id(),
                                            format!("Plan:\n(Click to vote or use buttons to get meal info)"),
                                            meal_plan.answers(),
                                        )
                                        .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                            keyboard.inline_keyboard(),
                                        )),
                                    PollKind::Plan { plan: meal_plan },
                                    keyboard.id,
                                ));
                            } else {
                                if meal_count < days_opt.unwrap_or(0) {
                                    request.message(cx.bot.send_message(
                                        cx.chat_id(),
                                        format!("Not enough Meals to generate plan!"),
                                    ));
                                } else {
                                    request.message(cx.bot.send_message(
                                        cx.chat_id(),
                                        format!("No Plan found, add with /plan <days>!"),
                                    ));
                                }
                            }
                        }

                        Command::List => {
                            let meal_btns: Vec<Vec<Button>> = state
                                .read()
                                .get_meals(cx.chat_id())
                                .iter()
                                .map(|meal| {
                                    vec![Button::new(
                                        meal.name.clone(),
                                        ButtonKind::DisplayListMeal { meal: meal.clone() },
                                    )]
                                })
                                .collect();
                            if meal_btns.len() > 0 {
                                request.message(
                                    cx.answer(format!("List:")).reply_markup(
                                        Keyboard::new(cx.chat_id())
                                            .buttons(meal_btns)
                                            .save(&state)
                                            .inline_keyboard(),
                                    ),
                                );
                            } else {
                                request.message(cx.answer(format!(
                                    "No meals saved!\n(save new meals with /new <meal name>)"
                                )));
                            }
                        }
                        Command::Rename(meal_name, new_name) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                state
                                    .write()
                                    .meal_entry(meal.id.clone())
                                    .or_insert(meal.clone())
                                    .rename(new_name.to_string());
                                request.add(meal.request(
                                    &cx,
                                    Some(format!("Renamed meal {} to {}", meal, new_name)),
                                    None,
                                ));
                                log::info!("Renamed meal {} to {}", meal_name, new_name)
                            }
                        }
                        Command::Rate(meal_name, new_rating) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                state
                                    .write()
                                    .meal_entry(meal.id.clone())
                                    .or_insert(meal.clone())
                                    .rate(Some(new_rating.clone()));
                                request.add(meal.request(
                                    &cx,
                                    Some(format!(
                                        "Changed rating of meal {} to {}",
                                        meal, new_rating
                                    )),
                                    None,
                                ));
                                log::info!(
                                    " Changed rating of meal {} to {}",
                                    meal_name,
                                    new_rating
                                )
                            }
                        }
                        Command::Tag(meal_name, new_tags) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                state
                                    .write()
                                    .meal_entry(meal.id.clone())
                                    .or_insert(meal.clone())
                                    .tag(new_tags.clone());
                                request.add(meal.request(
                                    &cx,
                                    Some(format!("Added tags to meal {}: {:?}", meal, new_tags)),
                                    None,
                                ));
                                log::info!("Added tags to meal {}: {:?}", meal_name, new_tags)
                            }
                        }
                        Command::TagRemove(meal_name, rem_tags) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let meal_tags = meal.tags.clone();
                                let mut new_tags = vec![];
                                for tag in meal_tags {
                                    if !rem_tags.contains(&tag) {
                                        new_tags.push(tag);
                                    }
                                }
                                state
                                    .write()
                                    .meal_entry(meal.id.clone())
                                    .or_insert(meal.clone())
                                    .set_tags(new_tags.clone());
                                request.add(meal.request(
                                    &cx,
                                    Some(format!(
                                        "Removed tags from meal {}: {:?}",
                                        meal, rem_tags
                                    )),
                                    None,
                                ));
                                log::info!("Removed tags from meal {}: {:?}", meal_name, rem_tags)
                            }
                        }
                        Command::Ref(meal_name, new_reference) => {
                            let meals = state
                                .read()
                                .get_meals_by_name(cx.chat_id(), meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                state
                                    .write()
                                    .meal_entry(meal.id.clone())
                                    .or_insert(meal.clone())
                                    .url(Some(new_reference.clone()));
                                request.add(meal.request(
                                    &cx,
                                    Some(format!(
                                        "Changed url of meal {} to {}",
                                        meal, new_reference
                                    )),
                                    None,
                                ));
                                log::info!("Changed url of meal {} to {}", meal_name, new_reference)
                            }
                        }
                        Command::Version => {
                            request.message(
                                cx.answer(format!("Bot version: {}", VERSION.unwrap_or("unknown"))),
                            );
                        }
                    }
                }
            }
            _ => {
                request.message(cx.answer(format!("No user found!")));
                return request;
            }
        }
        request.add(RequestKind::DeleteMessage(cx.delete_message()));
        request
    }

    pub fn execute(&self, state: &StateLock, cx: &ContextMessage) -> RequestResult {
        Self::run(self, state, cx)
    }
}

#[derive(BotCommand, Debug, Clone, Serialize, Deserialize)]
#[command(
    rename = "lowercase",
    description = "These photo commands are supported:"
)]
pub enum PhotoCommand {
    #[command(description = "Save a complete meal.", parse_with = "create_command")]
    New {
        meal_name: String,
        rating: Option<u8>,
        tags: Option<Vec<String>>,
        url: Option<String>,
    },
    Photo(String),
}

impl PhotoCommand {
    pub async fn run(
        command: &PhotoCommand,
        photos: &[PhotoSize],
        state: &StateLock,
        cx: &ContextMessage,
    ) {
        let mut request = RequestResult::default();
        let user_opt = cx.update.from();
        let whitelist: Vec<_> = state.read().get_whitelisted_users();
        match user_opt {
            Some(User {
                username: Some(username),
                id: user_id,
                ..
            }) => {
                if !whitelist.contains(&username.clone()) {
                    request.message(cx.answer(format!("User not whitelisted!")));
                } else {
                    match command {
                        PhotoCommand::New {
                            meal_name,
                            rating,
                            tags,
                            url,
                        } => {
                            for photo in photos.last() {
                                if let Ok(TgFile {
                                    file_path,
                                    file_unique_id,
                                    file_size,
                                    ..
                                }) = cx.bot.get_file(photo.file_id.clone()).send().await
                                {
                                    let file_r =
                                        File::create(format!("./images/{}.png", file_unique_id))
                                            .await;
                                    if let Ok(mut file) = file_r {
                                        match cx.bot.download_file(&file_path, &mut file).await {
                                            Ok(_) => log::info!(
                                                "Downloading File: {} | Size: {} ...",
                                                file_path,
                                                file_size
                                            ),
                                            Err(err) => log::warn!("{}", err),
                                        }
                                        let mut meal = Meal::new(meal_name, cx.chat_id(), *user_id);
                                        meal.rate(rating.clone())
                                            .tag(tags.clone().unwrap_or_default())
                                            .url(url.clone())
                                            .photo(photo.clone())
                                            .save(&state);
                                        RequestResult::default()
                                            .add(
                                                meal.request(
                                                    &cx,
                                                    None,
                                                    Some(
                                                        Keyboard::new(cx.chat_id())
                                                            .buttons(vec![
                                                                vec![Button::new(
                                                                    "Rate with Poll".into(),
                                                                    ButtonKind::PollRating {
                                                                        meal: meal.clone(),
                                                                    },
                                                                )],
                                                                button::save_meal_button_row(&meal),
                                                            ])
                                                            .save(&state),
                                                    ),
                                                ),
                                            )
                                            .send(&state)
                                            .await;
                                    }
                                }
                            }
                        }
                        PhotoCommand::Photo(meal_name) => {
                            for photo in photos.last() {
                                if let Ok(TgFile {
                                    file_path,
                                    file_unique_id,
                                    file_size,
                                    ..
                                }) = cx.bot.get_file(photo.file_id.clone()).send().await
                                {
                                    let file_r =
                                        File::create(format!("./images/{}.png", file_unique_id))
                                            .await;
                                    if let Ok(mut file) = file_r {
                                        match cx.bot.download_file(&file_path, &mut file).await {
                                            Ok(_) => log::info!(
                                                "Downloading File: {} | Size: {} ...",
                                                file_path,
                                                file_size
                                            ),
                                            Err(err) => log::warn!("{}", err),
                                        }
                                        let meals = state
                                            .read()
                                            .get_meals_by_name(cx.chat_id(), meal_name.clone());
                                        if meals.len() == 0 {
                                            RequestResult::default()
                                                .message(cx.answer(format!(
                                                    "No meal with name {}",
                                                    meal_name
                                                )))
                                                .send(&state)
                                                .await;
                                        }
                                        for meal in meals {
                                            state
                                                .write()
                                                .meal_entry(meal.id.clone())
                                                .or_insert(meal.clone())
                                                .photo(photo.clone());
                                            RequestResult::default()
                                                .add(meal.request(
                                                    &cx,
                                                    Some("Saved new photo!".to_string()),
                                                    None,
                                                ))
                                                .send(&state)
                                                .await;
                                            log::info!("Added photo to meal {}", meal_name,);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                request.message(cx.answer(format!("No user found!")));
            }
        }
        request.add(RequestKind::DeleteMessage(cx.delete_message()));
        request.send(state).await;
    }

    pub async fn execute(&self, photos: &[PhotoSize], state: &StateLock, cx: &ContextMessage) {
        Self::run(self, photos, state, cx).await;
    }
}
