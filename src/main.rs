use std::collections::HashMap;
use std::convert::Infallible;
use teloxide::{
    dispatching::{
        dialogue::{serializer::Bincode, RedisStorage, Storage},
        *,
    },
    prelude::*,
    types::*,
    utils::command::BotCommand,
};
use teloxide_macros::{teloxide, Transition};
mod db;
use db::{DBKeys, StoreHandler};
use derive_more::From;
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
    rating: u8,
    id: usize,
}

impl Meal {
    fn new(name: String) -> Self {
        Self {
            name: name,
            rating: 0,
            id: get_id(),
        }
    }

    fn rate(&mut self, rating: u8) -> Self {
        self.rating = rating;
        self.clone()
    }
}

struct State {
    sh: StoreHandler,
    buttons: HashMap<usize, Button>,
    meals: HashMap<usize, Meal>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sh: StoreHandler::new(),
            buttons: HashMap::new(),
            meals: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum ButtonKind {
    SaveMeal { meal_id: usize },
    RateMeal { meal_id: usize, rating: u8 },
    Cancel,
}

#[derive(Debug, Clone)]
struct Button {
    text: String,
    id: usize,
    kind: ButtonKind,
}

impl Button {
    pub fn new(text: String, kind: ButtonKind) -> Self {
        Self {
            text: text,
            id: get_id(),
            kind: kind,
        }
    }

    pub fn callback_button(&self) -> InlineKeyboardButton {
        InlineKeyboardButton::callback(self.text.to_uppercase(), self.id.to_string())
    }
}

fn rate_meal_buttons(
    state: &Arc<RwLock<State>>,
    max_rating: u8,
    rating: u8,
    meal_id: usize,
) -> Vec<InlineKeyboardButton> {
    (1..=max_rating)
        .into_iter()
        .map(|r| {
            let btn = Button::new(
                if r <= rating { "⭐" } else { "⚫" }.to_string(),
                ButtonKind::RateMeal { meal_id, rating: r },
            );
            state.write().buttons.insert(btn.id, btn.clone());
            btn.callback_button()
        })
        .collect()
}

fn save_meal_buttons(state: &Arc<RwLock<State>>, meal_id: usize) -> Vec<InlineKeyboardButton> {
    // create buttons and callback_buttons
    let save_button = Button::new("Save Meal".to_uppercase(), ButtonKind::SaveMeal { meal_id });
    let save_cb_button = save_button.callback_button();
    let cancel_button = Button::new("Cancel".to_uppercase(), ButtonKind::Cancel);
    let cancel_cb_button = cancel_button.callback_button();
    // save buttons
    state.write().buttons.insert(save_button.id, save_button);
    state
        .write()
        .buttons
        .insert(cancel_button.id, cancel_button);
    vec![save_cb_button, cancel_cb_button]
}

async fn handle_message(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<Message>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let text = cx.update.text().unwrap();
            let command = Command::parse(text, "name").unwrap();
            let _ = match command {
                Command::Help => cx.answer(Command::descriptions()).send().await,
                Command::NewMeal(meal_name) => {
                    // create new meal and store in meals
                    let meal = Meal::new(meal_name.clone());
                    let meal_id = meal.id;
                    // save meals
                    state.write().meals.insert(meal_id, meal.clone());
                    cx.answer(format!(
                        "MEAL: [{}]\nRATING: [{}]\n\nHow did it taste!",
                        meal.name, meal.rating
                    ))
                    .reply_markup(
                        InlineKeyboardMarkup::default()
                            .append_row(rate_meal_buttons(&state, MAX_RATING, 0, meal_id)),
                    )
                    .send()
                    .await
                }
                Command::Plan(days) => cx.answer_str(format!("Plan {} days:", days)).await,
            };
        })
        .await;
}

async fn handle_callback(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let data = cx.update.data.unwrap();
            let buttons = state.read().buttons.clone();
            let meals = state.read().meals.clone();
            let msg = cx.update.message.unwrap();
            let chat = ChatOrInlineMessage::Chat {
                chat_id: ChatId::Id(msg.chat_id()),
                message_id: msg.id,
            };
            let button_opt = buttons.get(&data.parse::<usize>().unwrap()).clone();
            if let Some(button) = button_opt {
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
                                        meal.name, meal.rating
                                    ),
                                )
                                .send()
                                .await;
                        } else {
                            let _ = cx
                                .bot
                                .edit_message_text(chat, "Failed to save, meal not found!")
                                .send()
                                .await;
                        }
                    }
                    ButtonKind::RateMeal { meal_id, rating } => {
                        {
                            let mut state_m = state.write();
                            state_m.meals.get_mut(&meal_id).unwrap().rating = rating
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
                                InlineKeyboardMarkup::default()
                                    .append_row(rate_meal_buttons(
                                        &state, MAX_RATING, rating, meal_id,
                                    ))
                                    .append_row(save_meal_buttons(&state, meal_id)),
                            )
                            .send()
                            .await;
                    }
                    ButtonKind::Cancel => {
                        let _ = cx.bot.edit_message_text(chat, "Canceled...").send().await;
                    }
                }
            }
            let _ = cx.bot.answer_callback_query(cx.update.id).send().await;
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting simple_commands_bot...");
    let bot = Bot::from_env();
    let state = Arc::new(RwLock::new(State::default()));
    let state_2 = state.clone();
    Dispatcher::new(bot)
        .messages_handler(|rx| handle_message(state_2, rx))
        .callback_queries_handler(|rx| handle_callback(state, rx))
        .dispatch()
        .await;
}
