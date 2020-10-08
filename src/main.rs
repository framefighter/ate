use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
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

pub const MAX_RATING: u8 = 5;
pub const BOT_NAME: &'static str = "eat_tracker_bot";

pub type StateLock = Arc<RwLock<State>>;
pub type ContextCallback = UpdateWithCx<CallbackQuery>;
pub type ContextMessage = UpdateWithCx<Message>;

async fn handle_message(state: StateLock, rx: DispatcherHandlerRx<Message>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            if let Some(text) = cx.update.text() {
                let parsed = Command::parse(text, BOT_NAME);
                if let Ok(command) = parsed {
                    command.execute(&state, &cx).send().await;
                } else if let Err(err) = parsed {
                    if let Err(err) = cx.answer(err.to_string()).send().await {
                        log::warn!("{}", err);
                    }
                }
            } else if let Some(photos) = cx.update.photo() {
                if let Some(last_photo) = photos.last() {
                    if let Some(caption) = cx.update.caption() {
                        let parsed = Command::parse(caption, BOT_NAME);
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
                                                    BOT_NAME,
                                                    file_path,
                                                    file_size
                                                );
                                            }
                                            command.execute(&state, &cx).send().await;
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
                                button.kind.execute(&state, &cx).send().await;
                                state.write().keyboards.remove(*keyboard_id);
                            }
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
            let query = cx.update.query;
            let mut results: Vec<InlineQueryResult> = vec![];
            if let Ok(command) = Command::parse(&query, BOT_NAME) {
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
            let total_votes = cx.update.total_voter_count;
            if total_votes > 0 {
                let v: Vec<_> = cx.update.question.split("|").collect();
                if let Some(meal_id) = v.get(0) {
                    let votes: Vec<(i32, i32)> = cx
                        .update
                        .options
                        .iter()
                        .enumerate()
                        .map(|(i, po)| ((i + 1) as i32, po.voter_count))
                        .collect();
                    let avg = votes.iter().fold(0, |sum, vote| sum + vote.0 * vote.1) / total_votes;
                    if let Some(meal) = state.write().meals.get_mut(meal_id.clone()) {
                        meal.rate(Some(((avg as u8) + meal.rating.unwrap_or(avg as u8)) / 2));
                    }
                }
            }
        })
        .await;
}

async fn handle_poll_answers(state: StateLock, rx: DispatcherHandlerRx<PollAnswer>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            dbg!(cx.update);
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("[{}] Starting...", BOT_NAME);
    let bot = BotBuilder::new().build();
    let state = Arc::new(RwLock::new(State::default()));
    let state_2 = state.clone();
    let state_3 = state.clone();
    let state_4 = state.clone();
    let state_5 = state.clone();
    Dispatcher::new(bot)
        .messages_handler(|rx| handle_message(state, rx))
        .callback_queries_handler(|rx| handle_callback(state_2, rx))
        .inline_queries_handler(|rx| handle_inline(state_3, rx))
        .polls_handler(|rx| handle_polls(state_4, rx))
        .poll_answers_handler(|rx| handle_poll_answers(state_5, rx))
        .dispatch()
        .await;
}
