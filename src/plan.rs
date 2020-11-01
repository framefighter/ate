use nanoid::nanoid;
use random_choice::random_choice;
use serde::{Deserialize, Serialize};

use crate::button::{Button, ButtonKind};
use crate::meal::Meal;
use crate::state::HasId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub meals: Vec<Meal>,
    pub days: usize,
    pub chat_id: i64,
    pub id: String,
}

impl HasId for Plan {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn chat_id(&self) -> i64 {
        self.chat_id
    }
}

impl Plan {
    pub fn new(chat_id: i64, meals: Vec<Meal>) -> Self {
        Self {
            chat_id,
            meals: meals.clone(),
            days: meals.len(),
            id: nanoid!(),
        }
    }

    pub fn gen(chat_id: i64, meals: Vec<Meal>, amount: usize) -> Self {
        let weights: Vec<f64> = meals
            .iter()
            .map(|meal| meal.rating.unwrap_or(1) as f64)
            .collect();
        let meal_plan: Vec<_> = random_choice()
            .random_choice_f64(&meals, &weights, amount)
            .into_iter()
            .map(|m| m.clone())
            .collect();
        let days = meal_plan.len();
        Self {
            chat_id,
            meals: meal_plan,
            days: days,
            id: nanoid!(),
        }
    }

    pub fn buttons(&self) -> Vec<Vec<Button>> {
        self.meals
            .as_slice()
            .chunks(4)
            .map(|row| {
                row.iter()
                    .map(|meal| {
                        Button::new(
                            meal.name.clone(),
                            ButtonKind::DisplayPlanMeal {
                                meal_id: meal.id.clone(),
                                plan_id: self.id.clone(),
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn answers(&self) -> Vec<String> {
        self.meals
            .iter()
            .map(|meal| {
                format!(
                    "{} ({}⭐)",
                    meal.name.to_uppercase(),
                    meal.rating.unwrap_or(1)
                )
            })
            .collect()
    }
}
