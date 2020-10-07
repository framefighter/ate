use nanoid::nanoid;
use teloxide::prelude::Request;
use teloxide::requests::{
    EditInlineMessageMedia, EditInlineMessageText, EditMessageMedia, EditMessageText,
};
use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup};

use crate::db::DBKeys;
use crate::keyboard::Keyboard;
use crate::meal::Meal;
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
}

impl ButtonKind {
    fn edit_callback_text(
        cx: &ContextCallback,
        text: String,
        reply_markup: InlineKeyboardMarkup,
    ) -> ButtonResult {
        let mut er = ButtonResult {
            edit_message: None,
            edit_photo: None,
            edit_inline_message: None,
            edit_inline_photo: None,
        };
        if let Some(msg) = &cx.update.message {
            er.message(
                cx.bot
                    .edit_message_text(ChatId::Id(msg.chat_id()), msg.id, text)
                    .reply_markup(reply_markup),
            );
        } else if let Some(id) = &cx.update.inline_message_id {
            er.inline_message(
                cx.bot
                    .edit_inline_message_text(id, text)
                    .reply_markup(reply_markup),
            );
        }
        er
    }

    pub fn run(button: &ButtonKind, state: &StateLock, cx: &ContextCallback) -> ButtonResult {
        match button {
            ButtonKind::SaveMeal { meal_id } => {
                let meals = state.read().meals.clone();
                let meal_opt = meals.get(meal_id).clone();
                if let Some(meal) = meal_opt {
                    let key: String = DBKeys::Meals.into();
                    state.write().sh.db.ladd(&key, &meal);
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
                let key: String = DBKeys::Meals.into();
                let text = if let Ok(b) = state.write().sh.db.lrem_value(&key, &meal) {
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
        }
    }
    pub fn execute(&self, state: &StateLock, cx: &ContextCallback) -> ButtonResult {
        Self::run(self, state, cx)
    }
}

pub struct ButtonResult {
    pub edit_message: Option<EditMessageText>,
    pub edit_photo: Option<EditMessageMedia>,
    pub edit_inline_message: Option<EditInlineMessageText>,
    pub edit_inline_photo: Option<EditInlineMessageMedia>,
}

impl ButtonResult {
    pub fn message(&mut self, edit_message: EditMessageText) {
        self.edit_message = Some(edit_message);
    }
    pub fn photo(&mut self, edit_photo: EditMessageMedia) {
        self.edit_photo = Some(edit_photo);
    }
    pub fn inline_message(&mut self, edit_inline_message: EditInlineMessageText) {
        self.edit_inline_message = Some(edit_inline_message);
    }
    pub fn inline_photo(&mut self, edit_inline_photo: EditInlineMessageMedia) {
        self.edit_inline_photo = Some(edit_inline_photo);
    }
    pub async fn send(&self) {
        if let Some(edit_message) = &self.edit_message {
            if let Err(err) = edit_message.send().await {
                log::warn!("{}", err);
            }
        }
        if let Some(edit_photo) = &self.edit_photo {
            if let Err(err) = edit_photo.send().await {
                log::warn!("{}", err);
            }
        }
        if let Some(edit_inline_message) = &self.edit_inline_message {
            if let Err(err) = edit_inline_message.send().await {
                log::warn!("{}", err);
            }
        }
        if let Some(edit_inline_photo) = &self.edit_inline_photo {
            if let Err(err) = edit_inline_photo.send().await {
                log::warn!("{}", err);
            }
        }
    }
}

impl Default for ButtonResult {
    fn default() -> Self {
        Self {
            edit_message: None,
            edit_photo: None,
            edit_inline_message: None,
            edit_inline_photo: None,
        }
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
