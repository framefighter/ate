use std::collections::HashMap;

use crate::db::StoreHandler;
use crate::keyboard::Keyboard;
use crate::meal::Meal;

pub struct State {
    pub sh: StoreHandler,
    pub keyboards: HashMap<String, Keyboard>,
    pub meals: HashMap<String, Meal>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sh: StoreHandler::new(),
            keyboards: HashMap::new(),
            meals: HashMap::new(),
        }
    }
}
