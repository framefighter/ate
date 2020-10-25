use random_choice::random_choice;
use teloxide::types::User;
use teloxide::utils::command::{BotCommand, ParseError};

use crate::button;
use crate::button::{Button, ButtonKind};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::request::{RequestKind, RequestResult};
use crate::{ContextMessage, StateLock, VERSION};

fn create_command(
    input: String,
) -> Result<(String, Option<u8>, Option<Vec<String>>, Option<String>), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            let meal_name = name.trim().to_string();
            if meal_name.len() > 0 {
                meal_name
            } else {
                return Err(ParseError::Custom("Provide a meal name!".into()));
            }
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

fn meal_name_command(input: String) -> Result<(String, String), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(name) = args.get(1) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a second argument!".into()));
        },
    ))
}

fn tag_meal_command(input: String) -> Result<(String, Vec<String>), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(tags_str) = args.get(1) {
            tags_str
                .trim()
                .split(" ")
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            vec![]
        },
    ))
}

fn rate_meal_command(input: String) -> Result<(String, u8), ParseError> {
    let args: Vec<_> = input.split(",").collect();
    Ok((
        if let Some(name) = args.get(0) {
            name.trim().to_string()
        } else {
            return Err(ParseError::Custom("Provide a meal name!".into()));
        },
        if let Some(rating_str) = args.get(1) {
            if let Ok(rating) = rating_str.trim().parse::<u8>() {
                rating
            } else {
                return Err(ParseError::Custom(
                    "Rating (2nd argument) has to be a number!".into(),
                ));
            }
        } else {
            return Err(ParseError::Custom("Provide a second argument!".into()));
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
    #[command(description = "Whitelist user.", parse_with = "meal_name_command")]
    Op(String, String),
    #[command(
        description = "Rename existing meal.",
        parse_with = "meal_name_command"
    )]
    Rename(String, String),
    #[command(
        description = "Change rating of existing meal.",
        parse_with = "rate_meal_command"
    )]
    Rate(String, u8),
    #[command(
        description = "Add tags to existing meal.",
        parse_with = "tag_meal_command"
    )]
    Tag(String, Vec<String>),
    #[command(
        description = "Remove tags from existing meal.",
        parse_with = "tag_meal_command"
    )]
    TagRemove(String, Vec<String>),
    #[command(
        description = "Edit reference of existing meal.",
        parse_with = "meal_name_command"
    )]
    Ref(String, String),
    #[command(description = "Add Photo to existing meal.")]
    Photo(String),
    #[command(description = "Get bot version.")]
    Version,
}

impl Command {
    pub fn run(command: &Command, state: &StateLock, cx: &ContextMessage) -> RequestResult {
        let mut request = RequestResult::default();
        let user = cx.update.from();
        let config = state.read().config.clone();
        match command {
            Command::Op(username, password) => {
                request.message(cx.answer(if password == &config.password {
                    state.write().whitelist_user(username.clone());
                    format!("Added user {} to whitelist.\nEnjoy!", username)
                } else {
                    format!("Wrong password: {}", password)
                }));
            }
            _ => {}
        }
        let whitelist: Vec<_> = state.read().get_whitelisted_users();
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
                            let meal = Meal::new(meal_name);
                            meal.save(&state);
                            request.add(
                                meal.request(
                                    &cx,
                                    Some("How did it taste?".to_string()),
                                    Some(
                                        Keyboard::new()
                                            .buttons(vec![button::rate_meal_button_row(
                                                0, &meal.id,
                                            )])
                                            .save(&state),
                                    ),
                                ),
                            );
                        }
                        Command::New {
                            meal_name,
                            rating,
                            tags,
                            url,
                        } => {
                            let mut meal = Meal::new(meal_name);
                            meal.rate(rating.clone())
                                .tag(tags.clone())
                                .url(url.clone())
                                .save(&state);
                            request.add(
                                meal.request(
                                    &cx,
                                    None,
                                    Some(
                                        Keyboard::new()
                                            .buttons(vec![
                                                vec![Button::new(
                                                    "Rate with Poll".into(),
                                                    ButtonKind::PollRating { meal: meal.clone() },
                                                )],
                                                button::save_meal_button_row(&meal.id),
                                            ])
                                            .save(&state),
                                    ),
                                ),
                            );
                        }
                        Command::Get(meal_name) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            for meal in meals {
                                request.add(
                                    meal.request(
                                        &cx,
                                        None,
                                        Some(
                                            Keyboard::new()
                                                .buttons(vec![vec![Button::new(
                                                    "Cancel".to_uppercase(),
                                                    ButtonKind::CancelMeal {
                                                        meal_id: meal.id.clone(),
                                                    },
                                                )]])
                                                .save(&state),
                                        ),
                                    ),
                                );
                            }
                        }
                        Command::Remove(meal_name) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request.message(
                                    cx.answer(format!("No meal with name {} found!", meal_name)),
                                );
                            }
                            for meal in meals {
                                if let Err(err) = state.write().remove_saved_meal(&meal) {
                                    log::warn!("{}", err);
                                }
                                request.add(meal.request(&cx, None, None));
                            }
                        }
                        Command::Plan(days) => {
                            let meals = state.read().get_saved_meals();
                            let meal_btns: Vec<Button> = meals
                                .iter()
                                .map(|meal| {
                                    Button::new(
                                        meal.name.clone(),
                                        ButtonKind::DisplayMeal { meal: meal.clone() },
                                    )
                                })
                                .collect();
                            let weights: Vec<f64> = meals
                                .iter()
                                .filter_map(|meal| meal.rating)
                                .map(|r| r as f64)
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
                                .get_saved_meals()
                                .iter()
                                .map(|meal| {
                                    vec![Button::new(
                                        meal.name.clone(),
                                        ButtonKind::DisplayMeal { meal: meal.clone() },
                                    )]
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
                        Command::Rename(meal_name, new_name) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let res = state.write().remove_saved_meal(&meal);
                                match res {
                                    Ok(rem) => {
                                        if rem {
                                            let mut new_meal = meal.clone();
                                            new_meal.name = new_name.clone();
                                            state.write().save_meal(&new_meal);
                                            request.add(meal.request(
                                                &cx,
                                                Some(format!(
                                                    "Renamed meal {} to {}",
                                                    meal_name, new_name
                                                )),
                                                None,
                                            ));
                                            log::info!("Renamed meal {} to {}", meal_name, new_name)
                                        }
                                    }
                                    Err(err) => log::warn!("{}", err),
                                }
                            }
                        }
                        Command::Rate(meal_name, new_rating) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let res = state.write().remove_saved_meal(&meal);
                                match res {
                                    Ok(rem) => {
                                        if rem {
                                            let mut new_meal = meal.clone();
                                            new_meal.rating = Some(new_rating.clone());
                                            state.write().save_meal(&new_meal);
                                            request.add(meal.request(
                                                &cx,
                                                Some(format!(
                                                    "Changed rating of meal {} to {}",
                                                    meal_name, new_rating
                                                )),
                                                None,
                                            ));
                                            log::info!(
                                                " Changed rating of meal {} to {}",
                                                meal_name,
                                                new_rating
                                            )
                                        }
                                    }
                                    Err(err) => log::warn!("{}", err),
                                }
                            }
                        }
                        Command::Tag(meal_name, new_tags) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let res = state.write().remove_saved_meal(&meal);
                                match res {
                                    Ok(rem) => {
                                        if rem {
                                            let mut new_meal = meal.clone();
                                            new_meal.tag(Some(new_tags.clone()));
                                            state.write().save_meal(&new_meal);
                                            request.add(meal.request(
                                                &cx,
                                                Some(format!(
                                                    "Added tags to meal {}: {:?}",
                                                    meal_name, new_tags
                                                )),
                                                None,
                                            ));
                                            log::info!(
                                                "Added tags to meal {}: {:?}",
                                                meal_name,
                                                new_tags
                                            )
                                        }
                                    }
                                    Err(err) => log::warn!("{}", err),
                                }
                            }
                        }
                        Command::TagRemove(meal_name, rem_tags) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let res = state.write().remove_saved_meal(&meal);
                                match res {
                                    Ok(rem) => {
                                        if rem {
                                            let mut new_meal = meal.clone();
                                            let meal_tags = meal.tags.clone();
                                            let mut new_tags = vec![];
                                            for tag in meal_tags {
                                                if !rem_tags.contains(&tag) {
                                                    new_tags.push(tag);
                                                }
                                            }
                                            new_meal.tags = new_tags.clone();
                                            state.write().save_meal(&new_meal);
                                            request.add(meal.request(
                                                &cx,
                                                Some(format!(
                                                    "Removed tags from meal {}: {:?}",
                                                    meal_name, rem_tags
                                                )),
                                                None,
                                            ));
                                            log::info!(
                                                "Removed tags from meal {}: {:?}",
                                                meal_name,
                                                rem_tags
                                            )
                                        }
                                    }
                                    Err(err) => log::warn!("{}", err),
                                }
                            }
                        }
                        Command::Ref(meal_name, new_reference) => {
                            let meals = state.read().get_saved_meals_by_name(meal_name.clone());
                            if meals.len() == 0 {
                                request
                                    .message(cx.answer(format!("No meal with name {}", meal_name)));
                            }
                            for meal in meals {
                                let res = state.write().remove_saved_meal(&meal);
                                match res {
                                    Ok(rem) => {
                                        if rem {
                                            let mut new_meal = meal.clone();
                                            new_meal.url = Some(new_reference.clone());
                                            state.write().save_meal(&new_meal);
                                            request.add(meal.request(
                                                &cx,
                                                Some(format!(
                                                    "Changed url of meal {} to {}",
                                                    meal_name, new_reference
                                                )),
                                                None,
                                            ));
                                            log::info!(
                                                "Changed url of meal {} to {}",
                                                meal_name,
                                                new_reference
                                            )
                                        }
                                    }
                                    Err(err) => log::warn!("{}", err),
                                }
                            }
                        }
                        Command::Photo(_) => {
                            request.message(cx.answer(format!("Attach a photo to your message!")));
                        }
                        Command::Version => {
                            request.message(
                                cx.answer(format!("Bot version: {}", VERSION.unwrap_or("unknown"))),
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
