use std::collections::HashMap;

use crate::db::{DBKeys, StoreHandler};
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
            sh: StoreHandler::default(),
            keyboards: HashMap::new(),
            meals: HashMap::new(),
            polls: HashMap::new(),
            config,
        }
    }

    pub fn save_meal(&mut self, meal: &Meal) {
        self.sh.db.ladd(&DBKeys::Meals.to_string(), meal);
        log::info!("Saving Meal: {:?}", meal);
    }

    pub fn remove_meal(&mut self, meal_id: String) {
        self.meals.remove(&meal_id);
        log::info!("Remove Meal");
    }

    pub fn remove_poll(&mut self, poll_id: String) {
        self.polls.remove(&poll_id);
        log::info!("Remove Poll");
    }

    pub fn remove_keyboard(&mut self, keyboard_id: String) {
        self.keyboards.remove(&keyboard_id);
        log::info!("Remove Keyboard");
    }
}
