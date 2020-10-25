use pickledb::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::{DBKeys, StoreHandler};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::poll::Poll;
use crate::Config;

pub struct State {
    sh: StoreHandler,
    tg: TgState,
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TgState {
    pub keyboards: HashMap<String, Keyboard>,
    pub meals: HashMap<String, Meal>,
    pub polls: HashMap<String, Poll>,
}

impl State {
    pub fn new(config: Config) -> Self {
        let sh = StoreHandler::new(config.backup);
        let tg_state_opt = sh.state_db.get::<TgState>(&DBKeys::State.to_string());
        let tg = match tg_state_opt {
            Some(tg_state) => {
                log::info!("Found existing telegram state!");
                tg_state
            }
            None => {
                log::info!("Create new telegram state!");
                TgState {
                    keyboards: HashMap::new(),
                    meals: HashMap::new(),
                    polls: HashMap::new(),
                }
            }
        };
        Self { sh, tg, config }
    }

    pub fn set_tg(&mut self, tg_state: TgState) -> &mut Self {
        self.tg = tg_state;
        self
    }

    pub fn save_tg(&mut self) {
        match self
            .sh
            .state_db
            .set(&DBKeys::State.to_string(), &self.tg.clone())
        {
            Ok(()) => log::info!("Saved state!"),
            Err(err) => log::warn!("{}", err),
        }
        log::debug!(
            "K: {} > {:#?} | M: {} | P: {}",
            self.keyboards().len(),
            self.keyboards(),
            self.meals().len(),
            self.polls().len()
        );
    }

    pub fn get_tg(&self) -> Option<TgState> {
        self.sh.state_db.get::<TgState>(&DBKeys::State.to_string())
    }

    pub fn meals(&self) -> &HashMap<String, Meal> {
        &self.tg.meals
    }
    pub fn keyboards(&self) -> &HashMap<String, Keyboard> {
        &self.tg.keyboards
    }
    pub fn polls(&self) -> &HashMap<String, Poll> {
        &self.tg.polls
    }
    pub fn meals_mut(&mut self) -> &mut HashMap<String, Meal> {
        &mut self.tg.meals
    }
    pub fn keyboards_mut(&mut self) -> &mut HashMap<String, Keyboard> {
        &mut self.tg.keyboards
    }
    pub fn polls_mut(&mut self) -> &mut HashMap<String, Poll> {
        &mut self.tg.polls
    }
    pub fn rate_meal(&mut self, meal_id: String, rating: u8) -> Result<Meal, ()> {
        match self.meals_mut().get_mut(&meal_id) {
            Some(meal) => {
                meal.rate(Some(rating));
                Ok(meal.clone())
            }
            None => Err(()),
        }
    }

    pub fn get_saved_meal(&self, meal_id: String) -> Option<Meal> {
        for item in self.sh.db.liter(&DBKeys::Meals.to_string()) {
            if let Some(meal) = item.get_item::<Meal>() {
                if meal.id == meal_id {
                    return Some(meal);
                }
            }
        }
        None
    }

    pub fn get_saved_meals(&self) -> Vec<Meal> {
        self.sh
            .db
            .liter(&DBKeys::Meals.to_string())
            .filter_map(|item| item.get_item::<Meal>())
            .collect()
    }

    pub fn get_saved_meals_by_name(&self, meal_name: String) -> Vec<Meal> {
        self.sh
            .db
            .liter(&DBKeys::Meals.to_string())
            .filter_map(|item| item.get_item::<Meal>())
            .filter(|meal| meal.name.to_uppercase() == meal_name.to_uppercase())
            .collect()
    }

    pub fn save_meal(&mut self, meal: &Meal) {
        self.sh.db.ladd(&DBKeys::Meals.to_string(), meal);
        log::info!("Saving Meal: {:?}", meal);
    }

    pub fn remove_saved_meal(&mut self, meal: &Meal) -> Result<bool, Error> {
        log::info!("Removing Meal: {:?}", meal);
        self.sh.db.lrem_value(&DBKeys::Meals.to_string(), meal)
    }

    pub fn remove_saved_meal_by_id(&mut self, meal_id: String) {
        if let Some(meal) = self.get_saved_meal(meal_id.clone()) {
            match self.sh.db.lrem_value(&DBKeys::Meals.to_string(), &meal) {
                Ok(rem) => log::info!("Removed Meal: {:?}? {}", meal, rem),
                Err(err) => log::warn!("{}", err),
            }
        } else {
            log::warn!("Meal to remove not found: {}", meal_id);
        }
    }

    pub fn whitelist_user(&mut self, username: String) {
        self.sh.db.ladd(&DBKeys::Whitelist.to_string(), &username);
        log::info!("Whitelisting User: {}", username);
    }

    pub fn get_whitelisted_users(&self) -> Vec<String> {
        self.sh
            .db
            .liter(&DBKeys::Whitelist.to_string())
            .filter_map(|item| item.get_item::<String>())
            .collect()
    }
}
