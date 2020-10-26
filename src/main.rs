use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use teloxide::types::File as TgFile;
use teloxide::{dispatching::*, prelude::*, types::*, utils::command::BotCommand, BotBuilder};
use tokio::fs::File;

mod button;
use button::{Button, ButtonKind};
mod db;
mod meal;
use meal::Meal;
mod command;
use command::Command;
mod keyboard;
use keyboard::Keyboard;
mod state;
use state::State;
mod poll;
mod request;
use request::{RequestKind, RequestResult};
mod plan;

pub const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

pub type StateLock = Arc<RwLock<State>>;
pub type ContextCallback = UpdateWithCx<CallbackQuery>;
pub type ContextMessage = UpdateWithCx<Message>;

async fn handle_message(state: StateLock, rx: DispatcherHandlerRx<Message>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let bot_name = state.read().config.name.clone();
            if let Some(text) = cx.update.text() {
                if !text.starts_with("/") {
                    return;
                }
                let parsed = Command::parse(text, bot_name);
                match parsed {
                    Ok(command) => command.execute(&state, &cx).send(&state).await,
                    Err(err) => {
                        if let Err(err) = cx.answer(err.to_string()).send().await {
                            log::warn!("{}", err);
                        }
                    }
                }
            } else if let Some(photos) = cx.update.photo() {
                if let Some(last_photo) = photos.last() {
                    if let Some(caption) = cx.update.caption() {
                        if !caption.starts_with("/") {
                            return;
                        }
                        let parsed = Command::parse(caption, bot_name);
                        match parsed {
                            Ok(command) => match &command {
                                Command::New {
                                    meal_name,
                                    rating,
                                    tags,
                                    url,
                                } => {
                                    if let Ok(TgFile {
                                        file_path,
                                        file_unique_id,
                                        file_size,
                                        ..
                                    }) = cx.bot.get_file(last_photo.file_id.clone()).send().await
                                    {
                                        let file_r = File::create(format!(
                                            "./images/{}.png",
                                            file_unique_id
                                        ))
                                        .await;
                                        if let Ok(mut file) = file_r {
                                            match cx.bot.download_file(&file_path, &mut file).await
                                            {
                                                Ok(_) => log::info!(
                                                    "Downloading File: {} | Size: {} ...",
                                                    file_path,
                                                    file_size
                                                ),
                                                Err(err) => log::warn!("{}", err),
                                            }
                                            let mut meal = Meal::new(meal_name);
                                            meal.rate(rating.clone())
                                                .tag(tags.clone())
                                                .url(url.clone())
                                                .photo(last_photo.clone())
                                                .save(&state);
                                            RequestResult::default()
                                                .add(
                                                    meal.request(
                                                        &cx,
                                                        None,
                                                        Some(
                                                            Keyboard::new()
                                                                .buttons(vec![
                                                                    vec![Button::new(
                                                                        "Rate with Poll".into(),
                                                                        ButtonKind::PollRating {
                                                                            meal: meal.clone(),
                                                                        },
                                                                    )],
                                                                    button::save_meal_button_row(
                                                                        &meal.id,
                                                                    ),
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
                                Command::Photo(meal_name) => {
                                    if let Ok(TgFile {
                                        file_path,
                                        file_unique_id,
                                        file_size,
                                        ..
                                    }) = cx.bot.get_file(last_photo.file_id.clone()).send().await
                                    {
                                        let file_r = File::create(format!(
                                            "./images/{}.png",
                                            file_unique_id
                                        ))
                                        .await;
                                        if let Ok(mut file) = file_r {
                                            match cx.bot.download_file(&file_path, &mut file).await
                                            {
                                                Ok(_) => log::info!(
                                                    "Downloading File: {} | Size: {} ...",
                                                    file_path,
                                                    file_size
                                                ),
                                                Err(err) => log::warn!("{}", err),
                                            }
                                            let meals = state
                                                .read()
                                                .get_saved_meals_by_name(meal_name.clone());
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
                                                let res = state.write().remove_saved_meal(&meal);
                                                match res {
                                                    Ok(rem) => {
                                                        if rem {
                                                            let mut new_meal = meal.clone();
                                                            new_meal.photo(last_photo.clone());
                                                            state.write().save_meal(&new_meal);
                                                            RequestResult::default()
                                                                .add(
                                                                    new_meal.request(
                                                                        &cx,
                                                                        Some(
                                                                            "Saved new photo!"
                                                                                .to_string(),
                                                                        ),
                                                                        None,
                                                                    ),
                                                                )
                                                                .send(&state)
                                                                .await;
                                                            log::info!(
                                                                "Added photo to meal {}",
                                                                meal_name,
                                                            );
                                                        }
                                                    }
                                                    Err(err) => log::warn!("{}", err),
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            },
                            Err(err) => {
                                if let Err(err) = cx.answer(err.to_string()).send().await {
                                    log::warn!("{}", err);
                                }
                            }
                        }
                    }
                }
            } else {
                log::warn!("Unhandled update!");
            }
        })
        .await;
}

async fn handle_callback(state: StateLock, rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let keyboards = state.read().keyboards().clone();
            match cx.update.clone() {
                CallbackQuery {
                    data: Some(data),
                    message: Some(message),
                    id,
                    ..
                } => {
                    let ids: Vec<_> = data.split(".").collect();
                    match *ids {
                        [keyboard_id, button_id] => match keyboards.get(keyboard_id) {
                            Some(keyboard) => {
                                if let Some(button) = keyboard.get_btn(button_id.to_string()) {
                                    button.kind.execute(&state, &cx).send(&state).await;
                                }
                                // state.write().keyboards_mut().remove(keyboard_id);
                            }
                            None => {
                                RequestResult::default()
                                    .add(RequestKind::CallbackAnswer(
                                        cx.bot
                                            .answer_callback_query(id)
                                            .text("Outdated buttons!\nPlease rerun command.")
                                            .show_alert(true),
                                    ))
                                    .add(RequestKind::EditReplyMarkup(
                                        cx.bot.edit_message_reply_markup(
                                            message.chat_id(),
                                            message.id,
                                        ),
                                    ))
                                    .send(&state)
                                    .await;
                            }
                        },
                        [..] => {}
                    }
                }
                _ => {}
            }
            if let Err(err) = cx.bot.answer_callback_query(cx.update.id).send().await {
                log::warn!("{}", err);
            }
        })
        .await;
}

fn meal_inline(meal: Meal) -> InlineQueryResult {
    if let Some(photo) = meal.photos.get(0) {
        InlineQueryResult::CachedPhoto(
            InlineQueryResultCachedPhoto::new(meal.id.to_string(), photo.file_id.clone())
                .caption(format!("{}", meal))
                .title(meal.name),
        )
    } else {
        InlineQueryResult::Article(
            InlineQueryResultArticle::new(
                meal.id.to_string(),
                meal.name.clone(),
                InputMessageContent::Text(InputMessageContentText::new(format!("{}", meal))),
            )
            .description(format!("{}", meal)),
        )
    }
}

async fn handle_inline(state: StateLock, rx: DispatcherHandlerRx<InlineQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let query = cx.update.query;
            let mut results: Vec<InlineQueryResult> = vec![];
            let meals_db: Vec<Meal> = state.read().get_saved_meals();
            meals_db.iter().for_each(|meal| {
                let matcher = SkimMatcherV2::default();
                if matcher.fuzzy_match(&meal.name, &query).is_some() || query.len() == 0 {
                    results.push(meal_inline(meal.clone()));
                }
            });
            if let Err(err) = cx
                .bot
                .answer_inline_query(cx.update.id, results)
                .cache_time(1)
                .send()
                .await
            {
                log::warn!("{}", err);
            }
        })
        .await;
}

async fn handle_polls(state: StateLock, rx: DispatcherHandlerRx<Poll>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let poll_opt = {
                let polls = state.read().polls().clone();
                let opt = polls.iter().find(|(_, p)| p.poll_id == cx.update.id);
                if let Some((_, poll)) = opt {
                    Some(poll.clone())
                } else {
                    None
                }
            };
            match poll_opt {
                Some(poll) => {
                    poll.handle_votes(&state, &cx).send(&state).await;
                }
                None => {
                    log::warn!("No poll with id: {}", cx.update.id);
                }
            }
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    password: String,
    token: String,
    name: String,
    backup: bool,
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Reading Config...");
    let config_str = fs::read_to_string("./config.json").expect("No config file found!");
    let config: Config = serde_json::from_str(&config_str).expect("Wrong config file!");
    let state = Arc::new(RwLock::new(State::new(config.clone())));
    let bot = BotBuilder::new().token(config.token).build();
    let state_2 = state.clone();
    let state_3 = state.clone();
    let state_4 = state.clone();

    log::info!("Dispatching Bot...");
    Dispatcher::new(bot)
        .messages_handler(|rx| handle_message(state, rx))
        .callback_queries_handler(|rx| handle_callback(state_2, rx))
        .inline_queries_handler(|rx| handle_inline(state_3, rx))
        .polls_handler(|rx| handle_polls(state_4, rx))
        .dispatch()
        .await;
}
