use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::fmt;
use teloxide::types::{InputFile, PhotoSize, ReplyMarkup};

use crate::keyboard::Keyboard;
use crate::request::RequestKind;
use crate::{ContextMessage, StateLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meal {
    pub name: String,
    pub rating: Option<u8>,
    pub id: String,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub photos: Vec<PhotoSize>,
    pub chat_id: i64,
    pub user_id: i32,
}

impl Meal {
    pub fn new(name: &String, chat_id: i64, user_id: i32,) -> Self {
        Self {
            chat_id,
            user_id,
            id: nanoid!(),
            name: name.to_string(),
            rating: None,
            url: None,
            tags: vec![],
            photos: vec![],
        }
    }

    pub fn rate(&mut self, rating: Option<u8>) -> &mut Self {
        self.rating = rating;
        self
    }

    pub fn tag(&mut self, tags: Option<Vec<String>>) -> &mut Self {
        self.tags.append(&mut tags.unwrap_or(vec![]));
        self
    }

    pub fn url(&mut self, url: Option<String>) -> &mut Self {
        self.url = url;
        self
    }

    pub fn photo(&mut self, photo: PhotoSize) -> &mut Self {
        self.photos.push(photo);
        self
    }

    pub fn save(&self, state: &StateLock) -> &Self {
        state.write().add_meal(self.chat_id, self.clone());
        self
    }

    pub fn request(
        &self,
        cx: &ContextMessage,
        sub_text: Option<String>,
        keyboard: Option<Keyboard>,
    ) -> RequestKind {
        let message_text = format!(
            "{}{}",
            self,
            if let Some(text) = sub_text {
                format!("\n\n{}", text)
            } else {
                "".to_string()
            }
        );
        if self.photos.len() > 0 {
            let mut req = cx
                .answer_photo(InputFile::FileId(
                    self.photos.last().unwrap().file_id.clone(),
                ))
                .caption(message_text);
            if let Some(keyboard_) = keyboard {
                req = req.reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                    keyboard_.inline_keyboard(),
                ));
            }
            RequestKind::Photo(req)
        } else {
            let mut req = cx.answer(message_text);
            if let Some(keyboard_) = keyboard {
                req = req.reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                    keyboard_.inline_keyboard(),
                ));
            }
            RequestKind::Message(req, false)
        }
    }
}

impl fmt::Display for Meal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name.to_uppercase();
        let rating = if let Some(rating) = self.rating {
            format!("\n{}", "⭐".repeat(rating as usize))
        } else {
            "".into()
        };
        let tags = if self.tags.len() > 0 {
            format!(
                "\n\n{} |",
                self.tags
                    .iter()
                    .fold(String::new(), |acc, arg| format!("{} | {}", acc, arg))
            )
        } else {
            "".into()
        };
        let url = if let Some(url) = self.url.clone() {
            format!("\n\n({})", url.to_string())
        } else {
            "".into()
        };
        write!(f, "{}{}{}{}", name, rating, tags, url)
    }
}
