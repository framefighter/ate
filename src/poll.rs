use teloxide::types::ChatId;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};

use crate::StateLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub id: String,
    pub poll_id: String,
    pub chat_id: ChatId,
    pub message_id: i32,
    pub meal_id: String,
}

impl Poll {
    pub fn new(poll_id: String, chat_id: ChatId, message_id: i32, meal_id: String) -> Self {
        Self {
            id: nanoid!(),
            poll_id,
            chat_id,
            message_id,
            meal_id,
        }
    }

    pub fn save(self, state: &StateLock) -> Self {
        state.write().polls.insert(self.id.clone(), self.clone());
        self
    }
}
