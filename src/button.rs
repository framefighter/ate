use nanoid::nanoid;
use teloxide::prelude::Request;
use teloxide::requests::{
    EditInlineMessageMedia, EditInlineMessageText, EditMessageMedia, EditMessageText, SendPoll,
    StopPoll,
};
use teloxide::types::{
    Chat, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, MediaKind, MediaPoll, Message,
    MessageCommon, MessageKind, ReplyMarkup,
};

use crate::db::DBKeys;
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::poll::Poll;
use crate::request::{RequestKind, RequestResult};
use crate::{ContextCallback, StateLock, MAX_RATING};

#[derive(Debug, Clone)]
pub struct Button {
    pub text: String,
    pub id: String,
    pub keyboard_id: Option<String>,
    pub kind: ButtonKind,
}

impl Button {
    pub fn new(text: String, kind: ButtonKind) -> Self {
        Self {
            id: nanoid!(),
            text,
            keyboard_id: None,
            kind,
        }
    }

    pub fn callback_button(&self) -> InlineKeyboardButton {
        if let Some(keyboard_id) = self.keyboard_id.clone() {
            InlineKeyboardButton::callback(
                self.text.to_uppercase(),
                format!("{}.{}", keyboard_id, self.id),
            )
        } else {
            InlineKeyboardButton::callback(self.text.to_uppercase(), format!(".{}", self.id))
        }
    }
}

#[derive(Debug, Clone)]
pub enum ButtonKind {
    DisplayMeal { meal: Meal },
    SaveMeal { meal_id: String },
    RateMeal { meal_id: String, rating: u8 },
    CancelMeal { meal_id: String },
    DeleteMeal { meal: Meal },
    PollRating { meal: Meal },
    SavePollRating { meal_id: String },
    CancelPollRating { meal_id: String },
}

impl ButtonKind {
    pub fn edit_callback_text(
        cx: &ContextCallback,
        text: String,
        reply_markup: InlineKeyboardMarkup,
    ) -> RequestResult {
        let mut result = RequestResult::default();
        if let Some(msg) = &cx.update.message {
            result.add(RequestKind::EditMessage(
                cx.bot
                    .edit_message_text(ChatId::Id(msg.chat_id()), msg.id, text.clone())
                    .reply_markup(reply_markup.clone()),
            ));
        }
        if let Some(id) = &cx.update.inline_message_id {
            result.add(RequestKind::EditInlineMessage(
                cx.bot
                    .edit_inline_message_text(id, text)
                    .reply_markup(reply_markup),
            ));
        }
        result
    }

    pub fn run(button: &ButtonKind, state: &StateLock, cx: &ContextCallback) -> RequestResult {
        match button {
            ButtonKind::SaveMeal { meal_id } => {
                let meals = state.read().meals.clone();
                let meal_opt = meals.get(meal_id).clone();
                if let Some(meal) = meal_opt {
                    state.write().sh.db.ladd(&DBKeys::Meals.to_string(), &meal);
                    state.write().meals.remove(meal_id);
                    let text = format!("{}\n\nSaved!", meal);
                    Self::edit_callback_text(&cx, text, Keyboard::new().inline_keyboard())
                } else {
                    let text = "Failed to save, meal not found!";
                    Self::edit_callback_text(
                        &cx,
                        text.to_string(),
                        Keyboard::new().inline_keyboard(),
                    )
                }
            }
            ButtonKind::CancelMeal { meal_id } => {
                state.write().meals.remove(meal_id);
                Self::edit_callback_text(
                    &cx,
                    "Canceled!".to_string(),
                    Keyboard::new().inline_keyboard(),
                )
            }
            ButtonKind::RateMeal { meal_id, rating } => {
                let mut text = "Not Found!".to_string();
                if let Some(meal) = state.write().meals.get_mut(meal_id) {
                    meal.rate(Some((*rating).min(1).max(MAX_RATING)));
                    text = format!("{}\n\nChange Rating or Save your Meal!", meal,);
                }
                Self::edit_callback_text(
                    &cx,
                    text,
                    Keyboard::new()
                        .buttons(vec![
                            rate_meal_button_row(*rating, meal_id.clone()),
                            save_meal_button_row(meal_id.clone()),
                        ])
                        .save(&state)
                        .inline_keyboard(),
                )
            }
            ButtonKind::DeleteMeal { meal } => {
                let text = if let Ok(b) = state
                    .write()
                    .sh
                    .db
                    .lrem_value(&DBKeys::Meals.to_string(), &meal)
                {
                    if b {
                        format!("{}\n\nRemoved!", meal,)
                    } else {
                        format!("{}\n\nNot Found!", meal,)
                    }
                } else {
                    format!("{}\n\nSomething went wrong!", meal,)
                };
                Self::edit_callback_text(&cx, text, Keyboard::new().inline_keyboard())
            }
            ButtonKind::DisplayMeal { meal } => Self::edit_callback_text(
                &cx,
                format!("{}", meal),
                Keyboard::new()
                    .buttons(vec![delete_meal_button_row(meal.clone())])
                    .save(&state)
                    .inline_keyboard(),
            ),
            ButtonKind::PollRating { meal } => {
                let mut result = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let answers: Vec<String> = (0..MAX_RATING)
                        .into_iter()
                        .map(|r| "⭐".repeat(r as usize + 1))
                        .collect();
                    result.add(RequestKind::Poll(
                        cx.bot
                            .send_poll(
                                ChatId::Id(message.chat_id()),
                                format!("Rate the meal: {}", meal.name.to_uppercase()),
                                answers,
                            )
                            .open_period(60)
                            .reply_to_message_id(message.id)
                            .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                Keyboard::new()
                                    .buttons(vec![save_poll_button_row(meal.clone())])
                                    .save(&state)
                                    .inline_keyboard(),
                            )),
                        meal.clone(),
                    ));
                }
                result
            }
            ButtonKind::SavePollRating { meal_id } => {
                let mut result = RequestResult::default();
                if let Some((_, poll)) = state
                    .read()
                    .polls
                    .iter()
                    .find(|(_, p)| &p.meal_id == meal_id)
                {
                    result.add(RequestKind::StopPoll(
                        cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                    ));
                }
                result
            }
            ButtonKind::CancelPollRating { meal_id } => {
                let mut result = RequestResult::default();
                if let Some((_, mut poll)) = state
                    .write()
                    .polls
                    .iter_mut()
                    .find(|(_, p)| &p.meal_id == meal_id)
                {
                    poll.is_canceled = true;
                    result.add(RequestKind::StopPoll(
                        cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                    ));
                }
                result
            }
        }
    }
    pub fn execute(&self, state: &StateLock, cx: &ContextCallback) -> RequestResult {
        Self::run(self, state, cx)
    }
}

pub fn rate_meal_button_row(rating: u8, meal_id: String) -> Vec<Button> {
    (1..=MAX_RATING)
        .into_iter()
        .map(|r| {
            Button::new(
                if r <= rating { "⭐" } else { "⚫" }.to_string(),
                ButtonKind::RateMeal {
                    meal_id: meal_id.clone(),
                    rating: r,
                },
            )
        })
        .collect()
}

pub fn save_meal_button_row(meal_id: String) -> Vec<Button> {
    let save_button = Button::new(
        "Save Meal".to_uppercase(),
        ButtonKind::SaveMeal {
            meal_id: meal_id.clone(),
        },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelMeal {
            meal_id: meal_id.clone(),
        },
    );
    vec![save_button, cancel_button]
}

pub fn save_poll_button_row(meal: Meal) -> Vec<Button> {
    let save_button = Button::new(
        "Save Meal".to_uppercase(),
        ButtonKind::SavePollRating {
            meal_id: meal.id.clone(),
        },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelPollRating {
            meal_id: meal.id.clone(),
        },
    );
    vec![save_button, cancel_button]
}

pub fn delete_meal_button_row(meal: Meal) -> Vec<Button> {
    let delete_button = Button::new(
        "Delete Meal".to_uppercase(),
        ButtonKind::DeleteMeal { meal: meal.clone() },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelMeal {
            meal_id: meal.id.clone(),
        },
    );
    vec![delete_button, cancel_button]
}
