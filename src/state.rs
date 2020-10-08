use teloxide::types::Poll;
use std::collections::HashMap;

use crate::db::StoreHandler;
use crate::keyboard::Keyboard;
use crate::meal::Meal;

pub struct State {
    pub sh: StoreHandler,
    pub keyboards: HashMap<String, Keyboard>,
    pub meals: HashMap<String, Meal>,
    pub polls: HashMap<String, Poll>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sh: StoreHandler::new(),
            keyboards: HashMap::new(),
            meals: HashMap::new(),
            polls: HashMap::new(),
        }
    }
}
