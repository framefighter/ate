use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::fmt;
use teloxide::types::PhotoSize;

use crate::StateLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meal {
    pub name: String,
    pub rating: Option<u8>,
    pub id: String,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub photos: Vec<PhotoSize>,
}

impl Meal {
    pub fn new(name: String) -> Self {
        Self {
            id: nanoid!(),
            name: name,
            rating: None,
            url: None,
            tags: vec![],
            photos: vec![],
        }
    }

    pub fn rate(&mut self, rating: Option<u8>) -> Self {
        self.rating = rating;
        self.clone()
    }

    pub fn tag(&mut self, tags: Option<Vec<String>>) -> Self {
        self.tags.append(&mut tags.unwrap_or(vec![]));
        self.clone()
    }

    pub fn url(&mut self, url: Option<String>) -> Self {
        self.url = url;
        self.clone()
    }

    pub fn photo(&mut self, photo: PhotoSize) -> Self {
        self.photos.push(photo);
        self.clone()
    }

    pub fn save(&mut self, state: &StateLock) -> Self {
        state.write().meals.insert(self.id.clone(), self.clone());
        self.clone()
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
