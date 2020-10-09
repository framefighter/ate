use std::collections::HashMap;

use crate::db::{StoreHandler, DBKeys};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::poll::Poll;
use crate::Config;

pub struct State {
    pub sh: StoreHandler,
    pub keyboards: HashMap<String, Keyboard>,
    pub meals: HashMap<String, Meal>,
    pub polls: HashMap<String, Poll>,
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        Self {
            sh: StoreHandler::new(),
            keyboards: HashMap::new(),
            meals: HashMap::new(),
            polls: HashMap::new(),
            config,
        }
    }

    pub fn save_meal(&mut self, meal: &Meal) {
        self.sh.db.ladd(&DBKeys::Meals.to_string(), meal);
    }
}
