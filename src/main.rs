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
use db::StoreHandler;
use derive_more::From;
use serde::{Deserialize, Serialize};

#[derive(BotCommand)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "List all commands.")]
    Help,
    #[command(description = "Save a meal.")]
    Meal(String),
    #[command(description = "handle a username and an age.", parse_with = "split")]
    Plan(u8),
}

fn save_meal_keyboard(data: String) -> InlineKeyboardMarkup {
    let save_button = InlineKeyboardButton::callback("Save".to_uppercase(), data.to_lowercase());
    let cancel_button =
        InlineKeyboardButton::callback("Cancel".to_uppercase(), data.to_lowercase());
    InlineKeyboardMarkup::default().append_row(vec![save_button, cancel_button])
}

async fn handle_message(rx: DispatcherHandlerRx<Message>) {
    rx.for_each_concurrent(None, |cx| async move {
        let text = cx.update.text().unwrap();
        let command = Command::parse(text, "name").unwrap();
        let _ = match command {
            Command::Help => cx.answer(Command::descriptions()).send().await,
            Command::Meal(meal) => {
                cx.answer(format!("Save meal {}?", meal))
                    .reply_markup(save_meal_keyboard(text.to_string()))
                    .send()
                    .await
            }
            Command::Plan(days) => cx.answer_str(format!("Plan {} days:", days)).await,
        };
    })
    .await;
}

async fn handle_callback(rx: DispatcherHandlerRx<CallbackQuery>) {
    rx.for_each_concurrent(None, |cx| async move {
        let data = cx.update.data.unwrap();
        let command = Command::parse(&data, "name").unwrap();
        match command {
            Command::Meal(meal) => {
                let _ = cx
                    .bot
                    .send_message(
                        cx.update.message.unwrap().chat.id,
                        &format!("data: {}\n meal: {}", data, meal),
                    )
                    .send()
                    .await;
            }
            Command::Plan(days) => {
                let _ = cx
                    .bot
                    .send_message(
                        cx.update.message.unwrap().chat.id,
                        &format!("data: {}\n days: {}", data, days),
                    )
                    .send()
                    .await;
            }
            Command::Help => {}
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
    Dispatcher::new(bot)
        .callback_queries_handler(handle_callback)
        .messages_handler(handle_message)
        .dispatch()
        .await;
}
