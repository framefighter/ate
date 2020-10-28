use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};

use crate::button::{Button, ButtonKind};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TgState {
    pub keyboards: HashMap<String, Keyboard>,
    pub meals: HashMap<String, Meal>,
    pub polls: HashMap<String, Poll>,
    pub plans: HashMap<i64, Plan>,
}

impl Default for TgState {
    fn default() -> Self {
        Self {
            keyboards: HashMap::new(),
            meals: HashMap::new(),
            polls: HashMap::new(),
            plans: HashMap::new(),
        }
    }
}

impl State {
    pub fn new(config: Config) -> Self {
        let sh = StoreHandler::new(config.backup);
        let tg = TgState::default();
        Self { sh, tg, config }
    }

    pub fn find_meal(&self, id: &String) -> Option<Meal> {
        self.tg.meals.get(id).cloned()
    }
    pub fn meal_entry(&mut self, id: String) -> Entry<String, Meal> {
        self.tg.meals.entry(id)
    }
    pub fn get_meals(&self, chat_id: i64) -> Vec<Meal> {
        self.tg
            .meals
            .iter()
            .filter_map(|(_, meal)| {
                if meal.chat_id == chat_id {
                    Some(meal.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn get_meals_mut(&mut self, chat_id: i64) -> Vec<&mut Meal> {
        self.tg
            .meals
            .iter_mut()
            .filter_map(|(_, meal)| {
                if meal.chat_id == chat_id {
                    Some(meal)
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn get_all_meals(&self) -> Vec<Meal> {
        self.tg.meals.iter().map(|(_, meal)| meal.clone()).collect()
    }
    pub fn get_meals_by_name(&self, chat_id: i64, meal_name: String) -> Vec<Meal> {
        self.get_meals(chat_id)
            .into_iter()
            .filter(|meal| meal.name.to_uppercase() == meal_name.to_uppercase())
            .collect()
    }

    pub fn find_keyboard(&self, id: &String) -> Option<Keyboard> {
        self.tg.keyboards.get(id).cloned()
    }
    pub fn find_poll(&self, id: &String) -> Option<Poll> {
        self.tg.polls.get(id).cloned()
    }
    pub fn poll_entry(&mut self, id: String) -> Entry<String, Poll> {
        self.tg.polls.entry(id)
    }
    pub fn find_plan(&self, chat_id: &i64) -> Option<Plan> {
        self.tg.plans.get(chat_id).cloned()
    }

    pub fn add_meal(&mut self, meal: Meal) {
        self.tg.meals.insert(meal.id.clone(), meal);
    }
    pub fn add_keyboard(&mut self, keyboard: Keyboard) {
        self.tg.keyboards.insert(keyboard.id.clone(), keyboard);
    }
    pub fn add_poll(&mut self, poll: Poll) {
        self.tg.polls.insert(poll.id.clone(), poll);
    }
    pub fn add_plan(&mut self, plan: Plan) {
        self.tg.plans.insert(plan.chat_id, plan);
    }

    pub fn remove_meal(&mut self, meal_id: &String) -> Option<Meal> {
        self.tg.meals.remove(meal_id)
    }
    pub fn remove_keyboard(&mut self, keyboard_id: &String) {
        self.tg.keyboards.remove(keyboard_id);
    }
    pub fn remove_poll(&mut self, id: &String) {
        self.tg.polls.remove(id);
    }
    pub fn remove_plan(&mut self, chat_id: &i64) {
        self.tg.plans.remove(chat_id);
    }

    pub fn find_poll_by_poll_id(&self, poll_id: String) -> Option<Poll> {
        self.tg
            .polls
            .iter()
            .map(|(_, p)| p)
            .find(|poll| poll.poll_id == poll_id)
            .cloned()
    }

    pub fn find_poll_by_meal_id(&self, meal_id: &String) -> Option<Poll> {
        self.tg
            .polls
            .iter()
            .map(|(_, p)| p)
            .find(|poll| match &poll.poll_kind {
                PollKind::Meal { meal_id: id, .. } => id == meal_id,
                _ => false,
            })
            .cloned()
    }

    pub fn find_poll_by_plan_id(&self, plan_id: &String) -> Option<Poll> {
        self.tg
            .polls
            .iter()
            .map(|(_, p)| p)
            .find(|poll| match &poll.poll_kind {
                PollKind::Plan {
                    plan: Plan { id, .. },
                    ..
                } => id == plan_id,
                _ => false,
            })
            .cloned()
    }

    pub fn rate_meal(&mut self, meal_id: &String, rating: u8) -> Option<Meal> {
        self.meal_entry(meal_id.clone()).and_modify(|meal| {
            meal.rate(Some(rating));
        });
        self.find_meal(meal_id)
    }

    pub fn save(&mut self) {
        match self.sh.db.set(&DBKeys::State.to_string(), &self.tg) {
            Ok(_) => log::info!("Saved State"),
            Err(err) => log::warn!("Could not save state: ({})", err),
        }
    }
    pub fn load(&mut self) {
        self.tg = self
            .sh
            .db
            .get(&DBKeys::State.to_string())
            .unwrap_or_default();
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

    pub fn meal_buttons(&self, chat_id: i64) -> Vec<Vec<Button>> {
        self.get_meals(chat_id)
            .as_slice()
            .chunks(4)
            .map(|row| {
                row.iter()
                    .map(|meal| {
                        Button::new(
                            meal.name.clone(),
                            ButtonKind::DisplayListMeal {
                                meal_id: meal.id.clone(),
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
