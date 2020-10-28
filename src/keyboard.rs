use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::button::Button;
use crate::StateLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyboard {
    pub id: String,
    pub buttons: Vec<Vec<Button>>,
    pub chat_id: i64,
}

impl Keyboard {
    pub fn new(chat_id: i64) -> Self {
        Self {
            chat_id,
            id: nanoid!(),
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
                        btn.keyboard_id = Some(self.id.clone());
                        btn.clone()
                    })
                    .collect()
            })
            .collect();
        self
    }

    pub fn get_btn(&self, button_id: String) -> Option<&Button> {
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

    pub fn save(self, state: &StateLock) -> Self {
        if self.buttons.iter().flatten().count() > 0 {
            state.write().add_keyboard(self.clone());
        }
        self
    }

    pub fn remove(self, state: &StateLock) -> Self {
        state.write().remove_keyboard(self.id.clone());
        self
    }
}
