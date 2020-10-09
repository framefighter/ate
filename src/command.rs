use random_choice::random_choice;
use teloxide::types::{InputFile, User};
use teloxide::utils::command::{BotCommand, ParseError};

use crate::button;
use crate::button::{Button, ButtonKind};
use crate::db::DBKeys;
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::request::{RequestKind, RequestResult};
use crate::{ContextMessage, StateLock};

fn create_command(
    input: String,
) -> Result<(String, Option<u8>, Option<Vec<String>>, Option<String>), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(rating_str) = args.get(1) {
            if let Ok(rating) = rating_str.trim().parse::<u8>() {
                Some(rating)
            } else {
                return Err(ParseError::Custom(
                    "Rating (2nd argument) has to be a number!".into(),
                ));
            }
        } else {
            None
        },
        if let Some(tags_str) = args.get(2) {
            let tags: Vec<_> = tags_str
                .trim()
                .split(" ")
                .map(|s| s.trim().to_string())
                .collect();
            Some(tags)
        } else {
            None
        },
        if let Some(url_str) = args.get(3) {
            Some(url_str.trim().to_string())
        } else {
            None
        },
    ))
}
#[derive(BotCommand, Debug, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "List all commands.")]
    Help,
    #[command(description = "Save a meal step by step.")]
    NewMeal(String),
    #[command(description = "Save a complete meal.", parse_with = "create_command")]
    New {
        meal_name: String,
        rating: Option<u8>,
        tags: Option<Vec<String>>,
        url: Option<String>,
    },
    #[command(description = "Plan meals for given days.", parse_with = "split")]
    Plan(u8),
    #[command(description = "Get a saved meal's info.")]
    Get(String),
    #[command(description = "Remove a meal by name.")]
    Remove(String),
    #[command(description = "Get a list of all meals.")]
    List,
    #[command(description = "Whitelist user.", parse_with = "split")]
    Op { user: String, password: String },
}

impl Command {
    pub fn run(command: &Command, state: &StateLock, cx: &ContextMessage) -> RequestResult {
        let mut request = RequestResult::default();
        let user = cx.update.from();
        let config = state.read().config.clone();
        match command {
            Command::Op {
                user: username,
                password,
            } => {
                request.message(cx.answer(if password == &config.password {
                    state
                        .write()
                        .sh
                        .db
                        .ladd(&DBKeys::Whitelist.to_string(), &username);
                    format!("Added user {} to whitelist.\nEnjoy!", username)
                } else {
                    format!("Wrong password: {}", password)
                }));
            }
            _ => {}
        }
        let whitelist: Vec<_> = state
            .read()
            .sh
            .db
            .liter(&DBKeys::Whitelist.to_string())
            .filter_map(|item| item.get_item::<String>())
            .collect();
        match user {
            Some(User {
                username: Some(username),
                ..
            }) => {
                if !whitelist.contains(&username.clone()) {
                    request.add(RequestKind::Message(
                        cx.answer(format!("User not whitelisted!")),
                    ));
                    return request;
                } else {
                    match command {
                        Command::Op { .. } => {}
                        Command::Help => {
                            request.message(cx.answer(Command::descriptions()));
                        }
                        Command::NewMeal(meal_name) => {
                            let meal = Meal::new(meal_name.clone()).save(&state);
                            request.message(
                                cx.answer(format!("{}\n\nHow did it taste?", meal))
                                    .reply_markup(
                                        Keyboard::new()
                                            .buttons(vec![button::rate_meal_button_row(
                                                0,
                                                meal.id,
                                            )])
                                            .save(&state)
                                            .inline_keyboard(),
                                    ),
                            );
                        }
                        Command::New {
                            meal_name,
                            rating,
                            tags,
                            url,
                        } => {
                            let meal = Meal::new(meal_name.clone())
                                .rate(*rating)
                                .tag(tags.clone())
                                .url(url.clone())
                                .save(&state);
                            request.message(
                                cx.answer(format!("{}", meal)).reply_markup(
                                    Keyboard::new()
                                        .buttons(vec![
                                            vec![Button::new(
                                                "Rate with Poll".into(),
                                                ButtonKind::PollRating { meal: meal.clone() },
                                            )],
                                            button::save_meal_button_row(meal.id),
                                        ])
                                        .save(&state)
                                        .inline_keyboard(),
                                ),
                            );
                        }
                        Command::Get(meal_name) => {
                            let mut meal_v: Vec<Meal> = vec![];
                            state
                                .read()
                                .sh
                                .db
                                .liter(&DBKeys::Meals.to_string())
                                .for_each(|item| {
                                    let meal_opt = item.get_item::<Meal>();
                                    if let Some(meal_f) = meal_opt {
                                        if &meal_f.name == meal_name {
                                            meal_v.push(meal_f);
                                        }
                                    }
                                });
                            dbg!(meal_v.clone());
                            for meal in meal_v {
                                if meal.photos.len() > 0 {
                                    request.add(RequestKind::Photo(
                                        cx.answer_photo(InputFile::FileId(
                                            meal.photos[0].file_id.clone(),
                                        ))
                                        .caption(format!("{}", meal)),
                                    ));
                                } else {
                                    request.message(cx.answer(format!("{}", meal)));
                                }
                            }
                        }
                        Command::Remove(meal_name) => {
                            let mut meal_q: Option<Meal> = None;
                            state
                                .read()
                                .sh
                                .db
                                .liter(&DBKeys::Meals.to_string())
                                .for_each(|item| {
                                    let meal_opt = item.get_item::<Meal>();
                                    if let Some(meal_f) = meal_opt {
                                        if &meal_f.name == meal_name {
                                            meal_q = Some(meal_f);
                                        }
                                    }
                                });
                            if let Some(meal) = meal_q {
                                if let Err(err) = state
                                    .write()
                                    .sh
                                    .db
                                    .lrem_value(&DBKeys::Meals.to_string(), &meal)
                                {
                                    log::warn!("{}", err);
                                }
                                if meal.photos.len() > 0 {
                                    request.add(RequestKind::Photo(
                                        cx.answer_photo(InputFile::FileId(
                                            meal.photos[0].file_id.clone(),
                                        ))
                                        .caption(format!("{}\n\nRemoved!", meal)),
                                    ));
                                } else {
                                    request.message(cx.answer(format!("{}\n\nRemoved!", meal)));
                                }
                            } else {
                                request.message(cx.answer(format!(
                                    "{}\n\nMeal not found!",
                                    meal_name.to_uppercase()
                                )));
                            }
                        }
                        Command::Plan(days) => {
                            let meal_btns: Vec<Button> = state
                                .read()
                                .sh
                                .db
                                .liter(&DBKeys::Meals.to_string())
                                .filter_map(|item| {
                                    let meal_opt = item.get_item::<Meal>();
                                    if let Some(meal) = meal_opt {
                                        Some(Button::new(
                                            meal.name.clone(),
                                            ButtonKind::DisplayMeal { meal: meal },
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            let weights: Vec<f64> = state
                                .read()
                                .sh
                                .db
                                .liter(&DBKeys::Meals.to_string())
                                .filter_map(|item| {
                                    let meal_opt = item.get_item::<Meal>();
                                    if let Some(meal) = meal_opt {
                                        if let Some(rating) = meal.rating {
                                            return Some(rating as f64);
                                        }
                                    }
                                    None
                                })
                                .collect();
                            let choices = random_choice().random_choice_f64(
                                &meal_btns,
                                &weights,
                                *days as usize,
                            );
                            request.message(
                                cx.answer(format!("Plan:\n(Click to see details)"))
                                    .reply_markup(
                                        Keyboard::new()
                                            .buttons(
                                                choices
                                                    .into_iter()
                                                    .map(|btn| vec![btn.clone()])
                                                    .collect(),
                                            )
                                            .save(&state)
                                            .inline_keyboard(),
                                    ),
                            );
                        }

                        Command::List => {
                            let meal_btns: Vec<Vec<Button>> = state
                                .read()
                                .sh
                                .db
                                .liter(&DBKeys::Meals.to_string())
                                .filter_map(|item| {
                                    let meal_opt = item.get_item::<Meal>();
                                    if let Some(meal) = meal_opt {
                                        Some(vec![Button::new(
                                            meal.name.clone(),
                                            ButtonKind::DisplayMeal { meal: meal },
                                        )])
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            request.message(
                                cx.answer(format!("List:")).reply_markup(
                                    Keyboard::new()
                                        .buttons(meal_btns)
                                        .save(&state)
                                        .inline_keyboard(),
                                ),
                            );
                        }
                    }
                }
            }
            _ => {
                request.add(RequestKind::Message(cx.answer(format!("No user found!"))));
                return request;
            }
        }
        request
    }

    pub fn execute(&self, state: &StateLock, cx: &ContextMessage) -> RequestResult {
        Command::run(self, state, cx)
    }
}
