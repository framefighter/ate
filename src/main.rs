use std::collections::HashMap;
use teloxide::types::File as TgFile;
use teloxide::{
    dispatching::*, prelude::*, requests::RequestWithFile, types::*, utils::command::BotCommand,
    BotBuilder,
};
mod db;
use db::{DBKeys, StoreHandler};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs::File;

pub static COUNTER: AtomicUsize = AtomicUsize::new(1);
pub fn get_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
const MAX_RATING: u8 = 5;
const BOT_NAME: &'static str = "eat_tracker_bot";

#[derive(BotCommand, Debug, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "List all commands.")]
    Help,
    #[command(description = "Save a meal step by step.")]
    NewMeal(String),
    #[command(description = "Save a complete meal.", parse_with = "split")]
    New {
        meal_name: String,
        rating: u8,
        tags: String,
        url: String,
    },
    #[command(description = "Plan meals for given days.", parse_with = "split")]
    Plan(u8),
    #[command(description = "Get a saved meal's info.")]
    Get(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meal {
    name: String,
    rating: Option<u8>,
    id: usize,
    url: Option<String>,
    tags: Vec<String>,
    photos: Vec<PhotoSize>,
}

impl Meal {
    fn new(name: String) -> Self {
        Self {
            id: get_id(),
            name: name,
            rating: None,
            url: None,
            tags: vec![],
            photos: vec![],
        }
    }

    fn rate(&mut self, rating: u8) -> Self {
        self.rating = Some(rating.max(1).min(MAX_RATING));
        self.clone()
    }

    fn tag(&mut self, tags: Vec<String>) -> Self {
        self.tags.append(&mut tags.clone());
        self.clone()
    }

    fn url(&mut self, url: String) -> Self {
        self.url = Some(url);
        self.clone()
    }

    fn photo(&mut self, photo: PhotoSize) -> Self {
        self.photos.push(photo);
        self.clone()
    }

    fn save(self, state: &Arc<RwLock<State>>) -> Self {
        state.write().meals.insert(self.id, self.clone());
        self
    }
}

impl fmt::Display for Meal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{}\n\n{} |\n\n({})",
            self.name.to_uppercase(),
            "⭐".repeat(self.rating.unwrap_or(0) as usize),
            self.tags
                .iter()
                .fold(String::new(), |acc, arg| format!("{} | {}", acc, arg)),
            self.url.clone().unwrap_or("".to_string())
        )
    }
}

struct State {
    sh: StoreHandler,
    keyboards: HashMap<usize, Keyboard>,
    meals: HashMap<usize, Meal>,
}

impl State {
    fn new() -> Self {
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
                if let Ok(command) = Command::parse(text, BOT_NAME) {
                    match command {
                        Command::Help => {
                            let _ = cx.answer(Command::descriptions()).send().await;
                        }
                        Command::NewMeal(meal_name) => {
                            let meal = Meal::new(meal_name).save(&state);
                            let _ = cx
                                .answer(format!("{}\n\nHow did it taste?", meal))
                                .reply_markup(
                                    Keyboard::new()
                                        .buttons(vec![rate_meal_button_row(0, meal.id)])
                                        .save(&state)
                                        .inline_keyboard(),
                                )
                                .send()
                                .await;
                        }
                        Command::Get(meal_name) => {
                            let key: String = DBKeys::Meals.into();
                            let mut meal_q: Option<Meal> = None;
                            state.read().sh.db.liter(&key).for_each(|item| {
                                let meal_opt = item.get_item::<Meal>();
                                if let Some(meal_f) = meal_opt {
                                    if meal_f.name == meal_name {
                                        meal_q = Some(meal_f);
                                    }
                                }
                            });
                            if let Some(meal) = meal_q {
                                if meal.photos.len() > 0 {
                                    let _ = cx
                                        .answer_photo(InputFile::FileId(
                                            meal.photos[0].file_id.clone(),
                                        ))
                                        .caption(format!("{}", meal))
                                        .send()
                                        .await;
                                } else {
                                    let _ = cx.answer(format!("{}", meal)).send().await;
                                }
                            }
                        }
                        Command::Plan(days) => {
                            let _ = cx.answer_str(format!("Plan {} days:", days)).await;
                        }
                        Command::New {
                            meal_name,
                            rating,
                            tags,
                            url,
                        } => {
                            let meal = Meal::new(meal_name)
                                .rate(rating)
                                .tag(tags.split(",").map(|s| s.to_string()).collect())
                                .url(url)
                                .save(&state);
                            let _ = cx
                                .answer(format!("{}", meal))
                                .reply_markup(
                                    Keyboard::new()
                                        .buttons(vec![save_meal_button_row(meal.id)])
                                        .save(&state)
                                        .inline_keyboard(),
                                )
                                .send()
                                .await;
                        }
                    };
                }
            } else if let Some(photos) = cx.update.photo() {
                if let Some(last_photo) = photos.last() {
                    if let Some(caption) = cx.update.caption() {
                        if let Ok(command) = Command::parse(caption, BOT_NAME) {
                            match command {
                                Command::New {
                                    meal_name,
                                    rating,
                                    tags,
                                    url,
                                } => {
                                    if let Ok(TgFile {
                                        file_path,
                                        file_unique_id,
                                        ..
                                    }) = cx.bot.get_file(last_photo.file_id.clone()).send().await
                                    {
                                        let file_r = File::create(format!(
                                            "./images/{}.png",
                                            file_unique_id
                                        ))
                                        .await;
                                        if let Ok(mut file) = file_r {
                                            let _ =
                                                cx.bot.download_file(&file_path, &mut file).await;
                                            let meal = Meal::new(meal_name)
                                                .rate(rating)
                                                .tag(
                                                    tags.split(",")
                                                        .map(|s| s.to_string())
                                                        .collect(),
                                                )
                                                .url(url)
                                                .photo(last_photo.clone())
                                                .save(&state);
                                            let _ = cx
                                                .answer(format!("{}", meal))
                                                .reply_markup(
                                                    Keyboard::new()
                                                        .buttons(vec![save_meal_button_row(
                                                            meal.id,
                                                        )])
                                                        .save(&state)
                                                        .inline_keyboard(),
                                                )
                                                .send()
                                                .await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        })
        .await;
}

async fn edit_callback_text(
    cx: &UpdateWithCx<CallbackQuery>,
    text: String,
    reply_markup: InlineKeyboardMarkup,
) {
    if let Some(msg) = &cx.update.message {
        let _ = cx
            .bot
            .edit_message_text(ChatId::Id(msg.chat_id()), msg.id, text)
            .reply_markup(reply_markup)
            .send()
            .await;
    } else if let Some(id) = &cx.update.inline_message_id {
        let _ = cx
            .bot
            .edit_inline_message_text(id, text)
            .reply_markup(reply_markup)
            .send()
            .await;
    }
}

async fn handle_callback(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let keyboards = state.read().keyboards.clone();
            let meals = state.read().meals.clone();
            if let Some(data) = cx.update.data.clone() {
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
                                                    let text = format!("{}\n\nSaved!", meal);
                                                    edit_callback_text(
                                                        &cx,
                                                        text,
                                                        Keyboard::new().inline_keyboard(),
                                                    )
                                                    .await;
                                                } else {
                                                    let text = "Failed to save, meal not found!";
                                                    edit_callback_text(
                                                        &cx,
                                                        text.to_string(),
                                                        Keyboard::new().inline_keyboard(),
                                                    )
                                                    .await;
                                                }
                                            }
                                            ButtonKind::CancelMeal { meal_id } => {
                                                state.write().meals.remove(&meal_id);
                                                edit_callback_text(
                                                    &cx,
                                                    "Canceled!".to_string(),
                                                    Keyboard::new().inline_keyboard(),
                                                )
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
                                                let text = format!(
                                                    "{}\n\nChange Rating or Save your Meal!",
                                                    state.read().meals.get(&meal_id).unwrap(),
                                                );
                                                edit_callback_text(
                                                    &cx,
                                                    text,
                                                    Keyboard::new()
                                                        .buttons(vec![
                                                            rate_meal_button_row(rating, meal_id),
                                                            save_meal_button_row(meal_id),
                                                        ])
                                                        .save(&state)
                                                        .inline_keyboard(),
                                                )
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
        .title(meal.name)
        .reply_markup(keyboard.inline_keyboard()),
    )
}

fn meal_photo(meal: Meal, keyboard: Keyboard) -> InlineQueryResult {
    InlineQueryResult::CachedPhoto(
        InlineQueryResultCachedPhoto::new(
            meal.id.to_string(),
            meal.photos.get(0).unwrap().file_id.clone(),
        )
        .description(format!("{}", meal))
        .caption(format!("{}", meal))
        .title(meal.name)
        .reply_markup(keyboard.inline_keyboard()),
    )
}

async fn handle_inline(state: Arc<RwLock<State>>, rx: DispatcherHandlerRx<InlineQuery>) {
    rx.map(|cx| (cx, state.clone()))
        .for_each_concurrent(None, |(cx, state)| async move {
            let key: String = DBKeys::Meals.into();
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
                            .tag(tags.split(",").map(|s| s.to_string()).collect())
                            .url(url)
                            .save(&state);
                        results.push(meal_article(
                            meal.clone(),
                            Keyboard::new()
                                .buttons(vec![save_meal_button_row(meal.id)])
                                .save(&state),
                        ));
                    }
                    _ => {}
                }
            } else {
                state.read().sh.db.liter(&key).for_each(|item| {
                    let matcher = SkimMatcherV2::default();
                    if let Some(meal) = item.get_item::<Meal>() {
                        if matcher.fuzzy_match(&meal.name, &query).is_some() {
                            if meal.photos.len() > 0 {
                                results.push(meal_photo(meal, Keyboard::new()));
                            } else {
                                results.push(meal_article(meal, Keyboard::new()));
                            }
                        }
                    }
                });
            }
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
    log::info!("{}", format!("Starting {}...", BOT_NAME));
    let bot = BotBuilder::new().build();
    let state = Arc::new(RwLock::new(State::new()));
    let state_2 = state.clone();
    let state_3 = state.clone();
    Dispatcher::new(bot)
        .messages_handler(|rx| handle_message(state, rx))
        .callback_queries_handler(|rx| handle_callback(state_2, rx))
        .inline_queries_handler(|rx| handle_inline(state_3, rx))
        .dispatch()
        .await;
}
