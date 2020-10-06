use std::collections::HashMap;
use teloxide::{dispatching::*, prelude::*, types::*, utils::command::BotCommand};
mod db;
use db::{DBKeys, StoreHandler};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub static COUNTER: AtomicUsize = AtomicUsize::new(1);
pub fn get_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
const MAX_RATING: u8 = 5;

#[derive(BotCommand)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "List all commands.")]
    Help,
    #[command(description = "Save a meal.")]
    NewMeal(String),
    #[command(description = "handle a username and an age.", parse_with = "split")]
    Plan(u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meal {
    name: String,
    rating: Option<u8>,
    id: usize,
    recipe: Option<String>,
}

impl Meal {
    fn new(name: String) -> Self {
        Self {
            id: get_id(),
            name: name,
            recipe: None,
            rating: None,
        }
    }

    fn rate(&mut self, rating: u8) -> Self {
        self.rating = Some(rating);
        self.clone()
    }

    fn save(self, state: &Arc<RwLock<State>>) -> Self {
        state.write().meals.insert(self.id, self.clone());
        self
    }
}

struct State {
    sh: StoreHandler,
    keyboards: HashMap<usize, Keyboard>,
    meals: HashMap<usize, Meal>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sh: StoreHandler::new(),
            keyboards: HashMap::new(),
            meals: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum ButtonKind {
    SaveMeal { meal_id: usize },
    RateMeal { meal_id: usize, rating: u8 },
    CancelMeal { meal_id: usize },
}

#[derive(Debug, Clone)]
struct Keyboard {
    id: usize,
    buttons: Vec<Vec<Button>>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            id: get_id(),
            buttons: vec![],
        }
    }

    pub fn buttons(mut self, buttons: Vec<Vec<Button>>) -> Self {
        self.buttons = buttons
            .clone()
            .iter_mut()
            .map(|row| {
                row.iter_mut()
                    .map(|btn| {
                        btn.keyboard_id = Some(self.id);
                        btn.clone()
                    })
                    .collect()
            })
            .collect();
        self
    }

    pub fn get_btn(&self, button_id: usize) -> Option<&Button> {
        self.buttons
            .iter()
            .flatten()
            .find(|btn| btn.id == button_id)
    }

    pub fn inline_keyboard(&self) -> InlineKeyboardMarkup {
        let keyboard: Vec<Vec<InlineKeyboardButton>> = self
            .buttons
            .iter()
            .map(|row| row.iter().map(|btn| btn.callback_button()).collect())
            .collect();
        InlineKeyboardMarkup::new(keyboard)
    }

    pub fn save(self, state: &Arc<RwLock<State>>) -> Self {
        state.write().keyboards.insert(self.id, self.clone());
        self
    }
}

#[derive(Debug, Clone)]
struct Button {
    text: String,
    id: usize,
    keyboard_id: Option<usize>,
    kind: ButtonKind,
}

impl Button {
    pub fn new(text: String, kind: ButtonKind) -> Self {
        Self {
            id: get_id(),
            text,
            keyboard_id: None,
            kind,
        }
    }

    pub fn callback_button(&self) -> InlineKeyboardButton {
        if let Some(keyboard_id) = self.keyboard_id {
            InlineKeyboardButton::callback(
                self.text.to_uppercase(),
                format!("{}.{}", keyboard_id, self.id),
            )
        } else {
            InlineKeyboardButton::callback(self.text.to_uppercase(), format!(".{}", self.id))
        }
    }
}

fn rate_meal_button_row(rating: u8, meal_id: usize) -> Vec<Button> {
    (1..=MAX_RATING)
        .into_iter()
        .map(|r| {
            Button::new(
                if r <= rating { "⭐" } else { "⚫" }.to_string(),
                ButtonKind::RateMeal { meal_id, rating: r },
            )
        })
        .collect()
}

fn save_meal_button_row(meal_id: usize) -> Vec<Button> {
    let save_button = Button::new("Save Meal".to_uppercase(), ButtonKind::SaveMeal { meal_id });
    let cancel_button = Button::new("Cancel".to_uppercase(), ButtonKind::CancelMeal { meal_id });
    vec![save_button, cancel_button]
}

async fn handle_message(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<Message>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            if let Some(text) = cx.update.text() {
                if let Ok(command) = Command::parse(text, "name") {
                    let _ = match command {
                        Command::Help => cx.answer(Command::descriptions()).send().await,
                        Command::NewMeal(meal_name) => {
                            let meal = Meal::new(meal_name).save(&state);
                            cx.answer(format!(
                                "MEAL: [{}]\nRATING: [{}]\n\nHow did it taste!",
                                meal.name,
                                meal.rating.unwrap_or(0)
                            ))
                            .reply_markup(
                                Keyboard::new()
                                    .buttons(vec![rate_meal_button_row(0, meal.id)])
                                    .save(&state)
                                    .inline_keyboard(),
                            )
                            .send()
                            .await
                        }
                        Command::Plan(days) => cx.answer_str(format!("Plan {} days:", days)).await,
                    };
                }
            }
        })
        .await;
}

async fn handle_callback(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let keyboards = state.read().keyboards.clone();
            let meals = state.read().meals.clone();
            if let Some(msg) = cx.update.message {
                let chat = ChatOrInlineMessage::Chat {
                    chat_id: ChatId::Id(msg.chat_id()),
                    message_id: msg.id,
                };
                if let Some(data) = cx.update.data {
                    let ids: Vec<_> = data.split(".").collect();
                    if let Some(keyboard_id_str) = ids.get(0) {
                        if let Some(button_id_str) = ids.get(1) {
                            if let Ok(keyboard_id) = keyboard_id_str.parse::<usize>() {
                                if let Ok(button_id) = button_id_str.parse::<usize>() {
                                    if let Some(keyboard) = keyboards.get(&keyboard_id) {
                                        if let Some(button) = keyboard.get_btn(button_id) {
                                            match button.kind {
                                                ButtonKind::SaveMeal { meal_id } => {
                                                    let meal_opt = meals.get(&meal_id).clone();
                                                    if let Some(meal) = meal_opt {
                                                        let key: String = DBKeys::Meals.into();
                                                        state.write().sh.db.ladd(&key, &meal);
                                                        state.write().meals.remove(&meal_id);

                                                        let _ = cx
                                                            .bot
                                                            .edit_message_text(
                                                                chat.clone(),
                                                                format!(
                                                            "MEAL: [{}]\nRATING: [{}]\n\nSaved!",
                                                            meal.name, meal.rating.unwrap_or(0)
                                                        ),
                                                            )
                                                            .send()
                                                            .await;
                                                    } else {
                                                        let _ = cx
                                                            .bot
                                                            .edit_message_text(
                                                                chat,
                                                                "Failed to save, meal not found!",
                                                            )
                                                            .send()
                                                            .await;
                                                    }
                                                }
                                                ButtonKind::CancelMeal { meal_id }=> {
                                                    state.write().meals.remove(&meal_id);
                                                    let _ = cx
                                                        .bot
                                                        .edit_message_text(chat, "Canceled...")
                                                        .send()
                                                        .await;
                                                }
                                                ButtonKind::RateMeal { meal_id, rating } => {
                                                    {
                                                        let mut state_m = state.write();
                                                        state_m
                                                            .meals
                                                            .get_mut(&meal_id)
                                                            .unwrap()
                                                            .rate(rating);
                                                    }
                                                    let _ = cx
                                                        .bot
                                                        .edit_message_text(
                                                            chat.clone(),
                                                            format!(
                                                                "MEAL: [{}]\nRATING: [{}]\n\nChange Rating or Save your Meal!",
                                                                state.read().meals.get(&meal_id).unwrap().name,
                                                                rating
                                                            ),
                                                        )
                                                        .reply_markup(
                                                            Keyboard::new()
                                                                .buttons(vec![
                                                                    rate_meal_button_row(
                                                                        rating, meal_id,
                                                                    ),
                                                                    save_meal_button_row(meal_id),
                                                                ])
                                                                .save(&state)
                                                                .inline_keyboard(),
                                                        )
                                                        .send()
                                                        .await;
                                                }
                                            }
                                            state.write().keyboards.remove(&keyboard_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = cx.bot.answer_callback_query(cx.update.id).send().await;
            }
            // dbg!(&state.read().meals);
            // dbg!(&state.read().keyboards);
        })
        .await;
}

fn meal_article(meal: Meal) -> InlineQueryResult {
    InlineQueryResult::Article(InlineQueryResultArticle::new(
        meal.id.to_string(),
        meal.name.clone(),
        InputMessageContent::Text(InputMessageContentText::new(format!(
            "MEAL: [{}]\nRATING: [{}]",
            meal.name,
            meal.rating.unwrap_or(0)
        ))),
    ))
}

async fn handle_inline(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<InlineQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let key: String = DBKeys::Meals.into();
            let results: Vec<_> = state
                .read()
                .sh
                .db
                .liter(&key)
                .map(|item| meal_article(item.get_item::<Meal>().unwrap()))
                .collect();
            dbg!(&results);
            let _ = cx
                .bot
                .answer_inline_query(cx.update.id, results)
                .send()
                .await;
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting eat_tracker_bot...");
    let bot = Bot::from_env();
    let state = Arc::new(RwLock::new(State::default()));
    let state_2 = state.clone();
    let state_3 = state.clone();
    Dispatcher::new(bot)
        .messages_handler(|rx| handle_message(state, rx))
        .callback_queries_handler(|rx| handle_callback(state_2, rx))
        .inline_queries_handler(|rx| handle_inline(state_3, rx))
        .dispatch()
        .await;
}
