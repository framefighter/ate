use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::db::{DBKeys, StoreHandler};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::plan::Plan;
use crate::poll::{Poll, PollKind};
use crate::Config;

pub struct State {
    sh: StoreHandler,
    pub tg: TgState,
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StateId(String, i64);

impl fmt::Display for StateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.0, self.1)
    }
}

#[derive(Clone, Debug)]
pub struct TgState {
    pub keyboards: HashMap<StateId, Keyboard>,
    pub meals: HashMap<StateId, Meal>,
    pub polls: HashMap<StateId, Poll>,
}

impl Default for TgState {
    fn default() -> Self {
        Self {
            keyboards: HashMap::new(),
            meals: HashMap::new(),
            polls: HashMap::new(),
        }
    }
}

impl State {
    pub fn new(config: Config) -> Self {
        let sh = StoreHandler::new(config.backup);
        let tg = TgState::default();
        Self { sh, tg, config }
    }

    pub fn find_meal(&mut self, chat_id: i64, id: String) -> Option<&mut Meal> {
        self.tg.meals.get_mut(&StateId(id, chat_id))
    }
    pub fn find_keyboard(&mut self, chat_id: i64, id: String) -> Option<&mut Keyboard> {
        self.tg.keyboards.get_mut(&StateId(id, chat_id))
    }
    pub fn find_poll(&mut self, chat_id: i64, id: String) -> Option<&mut Poll> {
        self.tg.polls.get_mut(&StateId(id, chat_id))
    }

    pub fn add_meal(&mut self, chat_id: i64, meal: Meal) {
        self.tg
            .meals
            .insert(StateId(meal.id.clone(), chat_id), meal);
    }
    pub fn add_keyboard(&mut self, chat_id: i64, keyboard: Keyboard) {
        self.tg
            .keyboards
            .insert(StateId(keyboard.id.clone(), chat_id), keyboard);
    }
    pub fn add_poll(&mut self, chat_id: i64, poll: Poll) {
        self.tg
            .polls
            .insert(StateId(poll.id.clone(), chat_id), poll);
    }

    pub fn remove_meal(&mut self, chat_id: i64, id: String) {
        self.tg.meals.remove(&StateId(id, chat_id));
    }
    pub fn remove_keyboard(&mut self, chat_id: i64, keyboard_id: String) {
        self.tg.keyboards.remove(&StateId(keyboard_id, chat_id));
    }
    pub fn remove_poll(&mut self, chat_id: i64, id: String) {
        self.tg.polls.remove(&StateId(id, chat_id));
    }

    pub fn find_poll_by_poll_id(&mut self, poll_id: String) -> Option<&mut Poll> {
        self.tg
            .polls
            .iter_mut()
            .map(|(_, p)| p)
            .find(|poll| poll.poll_id == poll_id)
    }

    pub fn find_poll_by_meal_id(&mut self, meal_id: String) -> Option<&mut Poll> {
        self.tg
            .polls
            .iter_mut()
            .map(|(_, p)| p)
            .find(|poll| match &poll.poll_kind {
                PollKind::Meal { meal_id: id, .. } => id == &meal_id,
                _ => false,
            })
    }

    pub fn rate_meal(&mut self, chat_id: i64, meal_id: String, rating: u8) -> Result<Meal, ()> {
        match self.find_meal(chat_id, meal_id) {
            Some(meal) => {
                meal.rate(Some(rating));
                Ok(meal.clone())
            }
            None => Err(()),
        }
    }

    pub fn get_saved_meal(&self, chat_id: i64, meal_id: String) -> Option<Meal> {
        self.sh.meal_db.get(&StateId(meal_id, chat_id).to_string())
    }

    pub fn get_saved_meals(&self, chat_id: i64) -> Vec<Meal> {
        self.sh
            .meal_db
            .iter()
            .filter_map(|item| match item.get_value::<Meal>() {
                Some(meal) => match meal {
                    Meal { chat_id: id, .. } => {
                        if id == chat_id {
                            Some(meal)
                        } else {
                            None
                        }
                    }
                },
                _ => None,
            })
            .collect()
    }

    pub fn get_all_saved_meals(&self) -> Vec<Meal> {
        self.sh
            .meal_db
            .iter()
            .filter_map(|item| item.get_value::<Meal>())
            .collect()
    }

    pub fn get_saved_meals_by_name(&self, chat_id: i64, meal_name: String) -> Vec<Meal> {
        self.get_saved_meals(chat_id)
            .into_iter()
            .filter(|meal| meal.name.to_uppercase() == meal_name.to_uppercase())
            .collect()
    }

    pub fn save_meal(&mut self, meal: &Meal) {
        match self
            .sh
            .meal_db
            .set(&StateId(meal.id.clone(), meal.chat_id).to_string(), meal)
        {
            Ok(()) => log::info!("Saving Meal: {:?}", meal),
            Err(err) => log::warn!("Error saving Meal: {}", err),
        }
    }

    pub fn remove_saved_meal(&mut self, meal: &Meal) {
        self.remove_saved_meal_by_id(meal.chat_id, meal.id.clone());
    }

    pub fn remove_saved_meal_by_id(&mut self, chat_id: i64, meal_id: String) {
        match self
            .sh
            .meal_db
            .rem(&StateId(meal_id.clone(), chat_id).to_string())
        {
            Ok(_) => log::info!("Removed Meal: {:?}", meal_id),
            Err(err) => log::warn!("Error removing Meal: {}", err),
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

    pub fn save_plan(&mut self, chat_id: i64, meal_plan: Plan) {
        match self.sh.plan_db.set(&chat_id.to_string(), &meal_plan) {
            Ok(()) => {}
            Err(err) => log::warn!("{}", err),
        }
    }

    pub fn get_plan(&self, chat_id: i64) -> Option<Plan> {
        self.sh.plan_db.get(&chat_id.to_string())
    }
}
