use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use teloxide::{
    dispatching::Dispatcher,
    prelude::*,
    types::{
        CallbackQuery, InlineQuery, InlineQueryResult, InlineQueryResultArticle,
        InlineQueryResultCachedPhoto, InputMessageContent, InputMessageContentText,
        Poll as TelePoll,
    },
    utils::command::BotCommand,
    BotBuilder,
};

mod button;
mod meal;
mod store_handler;
use meal::Meal;
mod command;
use command::{Command, PhotoCommand};
mod keyboard;
use keyboard::Keyboard;
mod state;
use state::State;
mod poll;
use poll::Poll;
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
                    Ok(command) => {
                        command.execute(&state, &cx).send(&state).await;
                    }
                    Err(err) => {
                        if let Err(err) = cx.answer(err.to_string()).send().await {
                            log::warn!("{}", err);
                        }
                    }
                }
            } else if let Some(photos) = cx.update.photo() {
                if let Some(caption) = cx.update.caption() {
                    if !caption.starts_with("/") {
                        return;
                    }
                    let parsed = PhotoCommand::parse(caption, bot_name);
                    match parsed {
                        Ok(command) => command.execute(photos, &state, &cx).await,
                        Err(err) => {
                            if let Err(err) = cx.answer(err.to_string()).send().await {
                                log::warn!("{}", err);
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
            match cx.update.clone() {
                CallbackQuery {
                    data: Some(data),
                    message: Some(message),
                    id,
                    ..
                } => {
                    let ids: Vec<_> = data.split(".").collect();
                    let chat_id = message.chat_id();
                    match *ids {
                        [keyboard_id, button_id] => {
                            let keyboard_opt: Option<Keyboard> =
                                state.read().get(&keyboard_id.to_string());
                            match keyboard_opt {
                                Some(keyboard) => {
                                    if let Some(button) = keyboard.get_btn(button_id.to_string()) {
                                        button.kind.execute(&state, &cx).send(&state).await;
                                    }
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
                                            cx.bot.edit_message_reply_markup(chat_id, message.id),
                                        ))
                                        .send(&state)
                                        .await;
                                }
                            }
                            match state.write().remove(&keyboard_id.to_string()) {
                                Ok(_) => log::debug!("Removed keyboard"),
                                Err(_) => log::warn!("Error removing keyboard"),
                            }
                        }
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

fn meal_inline(meal: &Meal) -> InlineQueryResult {
    if let Some(photo) = meal.photos.get(0) {
        InlineQueryResult::CachedPhoto(
            InlineQueryResultCachedPhoto::new(meal.id.to_string(), photo.file_id.clone())
                .caption(format!("{}", meal))
                .title(meal.name.clone()),
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
            let meals_db: Vec<Meal> = state.read().all();
            meals_db.iter().for_each(|meal| {
                let matcher = SkimMatcherV2::default();
                if matcher.fuzzy_match(&meal.name, &query).is_some() || query.len() == 0 {
                    results.push(meal_inline(meal));
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

async fn handle_polls(state: StateLock, rx: DispatcherHandlerRx<TelePoll>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let poll_opt: Option<Poll> = state
                .read()
                .find_all(|poll: &Poll| poll.poll_id == cx.update.id);
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
