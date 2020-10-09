use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use teloxide::types::File as TgFile;
use teloxide::{dispatching::*, prelude::*, types::*, utils::command::BotCommand, BotBuilder};
use tokio::fs::File;

mod db;
use db::DBKeys;
mod button;
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

pub type StateLock = Arc<RwLock<State>>;
pub type ContextCallback = UpdateWithCx<CallbackQuery>;
pub type ContextMessage = UpdateWithCx<Message>;

async fn handle_message(state: StateLock, rx: DispatcherHandlerRx<Message>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let bot_name = state.read().config.name.clone();
            if let Some(text) = cx.update.text() {
                let parsed = Command::parse(text, bot_name);
                if let Ok(command) = parsed {
                    command.execute(&state, &cx).send(&state).await;
                } else if let Err(err) = parsed {
                    if let Err(err) = cx.answer(err.to_string()).send().await {
                        log::warn!("{}", err);
                    }
                }
            } else if let Some(photos) = cx.update.photo() {
                if let Some(last_photo) = photos.last() {
                    if let Some(caption) = cx.update.caption() {
                        let parsed = Command::parse(caption, bot_name);
                        if let Ok(command) = parsed {
                            match &command {
                                Command::New { .. } => {
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
                                            if let Err(err) =
                                                cx.bot.download_file(&file_path, &mut file).await
                                            {
                                                log::warn!("{}", err);
                                            } else {
                                                log::info!(
                                                    "[{}] Downloading File: {} | Size: {} ...",
                                                    state.read().config.name,
                                                    file_path,
                                                    file_size
                                                );
                                            }
                                            command.execute(&state, &cx).send(&state).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else if let Err(err) = parsed {
                            if let Err(err) = cx.answer(err.to_string()).send().await {
                                log::warn!("{}", err);
                            }
                        }
                    }
                }
            } else {
                dbg!(cx.update);
            }
        })
        .await;
}

async fn handle_callback(state: StateLock, rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let keyboards = state.read().keyboards.clone();
            if let Some(data) = cx.update.data.clone() {
                let ids: Vec<_> = data.split(".").collect();
                if let Some(keyboard_id) = ids.get(0) {
                    if let Some(button_id) = ids.get(1) {
                        if let Some(keyboard) = keyboards.get(*keyboard_id) {
                            if let Some(button) = keyboard.get_btn(button_id.to_string()) {
                                button.kind.execute(&state, &cx).send(&state).await;
                                state.write().keyboards.remove(*keyboard_id);
                            }
                        } else if let Err(err) = cx
                            .bot
                            .answer_callback_query(cx.update.id.clone())
                            .text("Outdated buttons!\nPlease rerun command.")
                            .show_alert(true)
                            .send()
                            .await
                        {
                            log::warn!("{}", err);
                        }
                    }
                }
            }
            if let Err(err) = cx.bot.answer_callback_query(cx.update.id).send().await {
                log::warn!("{}", err);
            }
        })
        .await;
}

fn meal_article(meal: Meal, keyboard: Keyboard) -> InlineQueryResult {
    InlineQueryResult::Article(
        InlineQueryResultArticle::new(
            meal.id.to_string(),
            meal.name.clone(),
            InputMessageContent::Text(InputMessageContentText::new(format!("{}", meal))),
        )
        .description(format!("{}", meal))
        .reply_markup(keyboard.inline_keyboard()),
    )
}

fn meal_photo(meal: Meal, keyboard: Keyboard) -> InlineQueryResult {
    if let Some(photo) = meal.photos.get(0) {
        InlineQueryResult::CachedPhoto(
            InlineQueryResultCachedPhoto::new(meal.id.to_string(), photo.file_id.clone())
                .caption(format!("{}", meal))
                .title(meal.name)
                .reply_markup(keyboard.inline_keyboard()),
        )
    } else {
        meal_article(meal, keyboard)
    }
}

async fn handle_inline(state: StateLock, rx: DispatcherHandlerRx<InlineQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let bot_name = state.read().config.name.clone();
            let query = cx.update.query;
            let mut results: Vec<InlineQueryResult> = vec![];
            if let Ok(command) = Command::parse(&query, bot_name) {
                match command {
                    Command::New {
                        meal_name,
                        rating,
                        tags,
                        url,
                    } => {
                        let meal = Meal::new(meal_name)
                            .rate(rating)
                            .tag(tags)
                            .url(url)
                            .save(&state);
                        results.push(meal_article(
                            meal.clone(),
                            Keyboard::new()
                                .buttons(vec![button::save_meal_button_row(meal.id)])
                                .save(&state),
                        ));
                    }
                    _ => {}
                }
            } else {
                let meals_db: Vec<Option<Meal>> = state
                    .read()
                    .sh
                    .db
                    .liter(&DBKeys::Meals.to_string())
                    .map(|item| item.get_item::<Meal>())
                    .collect();
                meals_db.iter().for_each(|item| {
                    let matcher = SkimMatcherV2::default();
                    if let Some(meal) = item {
                        let keyboard = Keyboard::new()
                            .buttons(vec![button::delete_meal_button_row(meal.clone())])
                            .save(&state);
                        if matcher.fuzzy_match(&meal.name, &query).is_some() || query.len() == 0 {
                            if meal.photos.len() > 0 {
                                results.push(meal_photo(meal.clone(), keyboard));
                            } else {
                                results.push(meal_article(meal.clone(), keyboard));
                            }
                        }
                    }
                });
            }
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
                let s = state.read();
                let opt = s.polls.iter().find(|(_, p)| p.poll_id == cx.update.id);
                if let Some((_, poll)) = opt {
                    Some(poll.clone())
                } else {
                    None
                }
            };
            match poll_opt {
                Some(poll) => {
                    let meal_id = poll.meal_id.clone();
                    let total_votes = cx.update.total_voter_count;
                    if total_votes > 0 && cx.update.is_closed && !poll.is_canceled {
                        let votes: Vec<(i32, i32)> = cx
                            .update
                            .options
                            .iter()
                            .enumerate()
                            .map(|(i, po)| ((i + 1) as i32, po.voter_count))
                            .collect();
                        let avg =
                            votes.iter().fold(0, |sum, vote| sum + vote.0 * vote.1) / total_votes;
                        let meal_opt = {
                            let s = state.read();
                            let opt = s.meals.iter().find(|(_, meal)| meal.id == meal_id);
                            if let Some((_, meal)) = opt {
                                Some(meal.clone())
                            } else {
                                None
                            }
                        };
                        match meal_opt {
                            Some(mut meal) => {
                                meal.rate(Some(
                                    ((avg as u8) + meal.rating.unwrap_or(avg as u8)) / 2,
                                ));
                                state.write().meals.insert(meal.id.clone(), meal.clone());
                                state.write().save_meal(&meal);
                                log::info!("Poll closed: {}", meal.name);
                                RequestResult::default()
                                    .add(RequestKind::EditMessage(cx.bot.edit_message_text(
                                        poll.chat_id,
                                        poll.reply_message_id,
                                        format!("{}\n\nSaved!", meal),
                                    )))
                                    .send(&state)
                                    .await;
                                log::info!("Saving Meal: {:?}", meal);
                            }
                            None => {
                                log::info!(
                                    "No meal with id {} found for poll: {:?}",
                                    meal_id,
                                    poll
                                );
                            }
                        }
                    } else if poll.is_canceled {
                        RequestResult::default()
                            .add(RequestKind::EditMessage(cx.bot.edit_message_text(
                                poll.chat_id,
                                poll.reply_message_id,
                                format!("Poll Canceled!"),
                            )))
                            .send(&state)
                            .await;
                    }
                }
                None => {
                    log::info!("No poll with id: {}", cx.update.id);
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
