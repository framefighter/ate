use nanoid::nanoid;
use random_choice::random_choice;
use serde::{Deserialize, Serialize};

use crate::button::{Button, ButtonKind};
use crate::meal::Meal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub meals: Vec<Meal>,
    pub days: usize,
    pub id: String,
}

impl Plan {
    pub fn new(meals: Vec<Meal>) -> Self {
        Self {
            meals: meals.clone(),
            days: meals.len(),
            id: nanoid!(),
        }
    }

    pub fn gen(meals: Vec<Meal>, amount: usize) -> Self {
        let weights: Vec<f64> = meals
            .iter()
            .map(|meal| meal.rating.unwrap_or(1) as f64)
            .collect();
        let meal_plan: Vec<_> = random_choice()
            .random_choice_f64(&meals, &weights, amount)
            .into_iter()
            .map(|m| m.clone())
            .collect();
        Self {
            meals: meal_plan,
            days: amount,
            id: nanoid!(),
        }
    }

    pub fn buttons(&self) -> Vec<Vec<Button>> {
        self.meals
            .iter()
            .map(|meal| {
                vec![Button::new(
                    meal.name.clone(),
                    ButtonKind::DisplayPlanMeal {
                        meal: meal.clone(),
                        plan: self.clone(),
                    },
                )]
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
