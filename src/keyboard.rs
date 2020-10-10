use teloxide::types::{InlineKeyboardMarkup, InlineKeyboardButton};
use nanoid::nanoid;
use serde::{Serialize, Deserialize};

use crate::StateLock;
use crate::button::Button;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyboard {
    pub id: String,
    pub buttons: Vec<Vec<Button>>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
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
        state
            .write()
            .keyboards
            .insert(self.id.clone(), self.clone());
        self
    }
}
