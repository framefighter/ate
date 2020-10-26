use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use teloxide::dispatching::UpdateWithCx;
use teloxide::types::{
    ChatId, InlineKeyboardButton, InlineKeyboardMarkup, MediaKind, Message, MessageCommon,
    MessageKind, ReplyMarkup,
};

use crate::command::Command;
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::plan::Plan;
use crate::poll::PollKind;
use crate::request::{RequestKind, RequestResult};
use crate::{ContextCallback, StateLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    pub text: String,
    pub id: String,
    pub keyboard_id: Option<String>,
    pub kind: ButtonKind,
}

impl Button {
    pub fn new(text: String, kind: ButtonKind) -> Self {
        Self {
            id: nanoid!(),
            text,
            keyboard_id: None,
            kind,
        }
    }

    pub fn callback_button(&self) -> InlineKeyboardButton {
        if let Some(keyboard_id) = self.keyboard_id.clone() {
            InlineKeyboardButton::callback(
                self.text.to_uppercase(),
                format!("{}.{}", keyboard_id, self.id),
            )
        } else {
            InlineKeyboardButton::callback(self.text.to_uppercase(), format!(".{}", self.id))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonKind {
    DisplayPlanMeal { meal: Meal, plan: Plan },
    DisplayListMeal { meal: Meal },
    ShowList,
    ShowPlan { plan: Plan },
    RerollPlan { plan: Plan },
    ClearVotes { plan: Plan },
    SaveMeal { meal_id: String },
    RateMeal { meal_id: String, rating: u8 },
    CancelMeal { meal_id: String },
    DeleteMeal { meal: Meal },
    PollRating { meal: Meal },
    SavePollRating { meal_id: String },
    CancelPollRating { meal_id: String },
    CommandButton { command: Command },
    PinMessage,
    DeleteMessage,
}

impl ButtonKind {
    pub fn edit_callback_text(
        cx: &ContextCallback,
        text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> RequestResult {
        let mut result = RequestResult::default();
        if let Some(msg) = &cx.update.message {
            match msg {
                Message {
                    kind:
                        MessageKind::Common(MessageCommon {
                            media_kind: MediaKind::Photo(_),
                            ..
                        }),
                    ..
                } => {
                    let mut edit = cx
                        .bot
                        .edit_message_caption(ChatId::Id(msg.chat_id()), msg.id)
                        .caption(text);
                    if let Some(keyboard) = reply_markup {
                        edit = edit.reply_markup(keyboard);
                    }
                    result.add(RequestKind::EditCaption(edit));
                }
                _ => {
                    let mut edit =
                        cx.bot
                            .edit_message_text(ChatId::Id(msg.chat_id()), msg.id, text.clone());
                    if let Some(keyboard) = reply_markup {
                        edit = edit.reply_markup(keyboard);
                    }
                    result.add(RequestKind::EditMessage(edit));
                }
            }
        } else if let Some(id) = &cx.update.inline_message_id {
            let mut edit = cx.bot.edit_inline_message_text(id, text.clone());
            if let Some(keyboard) = reply_markup {
                edit = edit.reply_markup(keyboard);
            }
            result.add(RequestKind::EditInlineMessage(edit));
        }
        result
    }

    pub fn run(button: &ButtonKind, state: &StateLock, cx: &ContextCallback) -> RequestResult {
        match button {
            ButtonKind::SaveMeal { meal_id } => {
                let meals = state.read().meals().clone();
                let meal_opt = meals.get(meal_id).clone();
                match meal_opt {
                    Some(meal) => {
                        state.write().save_meal(&meal);
                        state.write().meals_mut().remove(&meal.id);
                        Self::edit_callback_text(&cx, format!("{}\n\nSaved!", meal), None)
                    }
                    None => Self::edit_callback_text(
                        &cx,
                        "Failed to save, meal not found!".to_string(),
                        None,
                    ),
                }
            }
            ButtonKind::PinMessage => {
                let mut result = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    result.add(RequestKind::Pin(
                        cx.bot.pin_chat_message(message.chat_id(), message.id),
                    ));
                }
                result
            }
            ButtonKind::CancelMeal { meal_id } => {
                state.write().meals_mut().remove(meal_id);
                Self::edit_callback_text(&cx, "Canceled!".to_string(), None)
            }
            ButtonKind::RateMeal { meal_id, rating } => {
                let rated_meal = state.write().rate_meal(meal_id.clone(), rating.clone());
                log::info!("Rated meal: {:?}", rated_meal);
                Self::edit_callback_text(
                    &cx,
                    match rated_meal {
                        Ok(meal) => format!("{}\n\nChange rating or save your meal!", meal),
                        Err(()) => {
                            log::warn!("Meal not found: {}", meal_id);
                            "No meal to rate found!".to_string()
                        }
                    },
                    Some(
                        Keyboard::new()
                            .buttons(vec![
                                rate_meal_button_row(*rating, meal_id),
                                save_meal_button_row(meal_id),
                            ])
                            .save(state)
                            .inline_keyboard(),
                    ),
                )
            }
            ButtonKind::DeleteMeal { meal } => Self::edit_callback_text(
                &cx,
                match state.write().remove_saved_meal(&meal) {
                    Ok(b) => {
                        if b {
                            format!("{}\n\nRemoved!", meal,)
                        } else {
                            format!("{}\n\nNot Found!", meal,)
                        }
                    }
                    Err(err) => {
                        log::warn!("Delete Meal: {}", err);
                        format!("{}\n\nSomething went wrong!", meal)
                    }
                },
                None,
            ),
            ButtonKind::DisplayPlanMeal { meal, .. } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    request.message(
                        cx.bot
                            .send_message(message.chat_id(), format!("{}", meal))
                            .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                Keyboard::new()
                                    .buttons(vec![vec![Button::new(
                                        format!("Back"),
                                        ButtonKind::DeleteMessage,
                                    )]])
                                    .save(state)
                                    .inline_keyboard(),
                            )),
                    );
                }
                request
            }
            ButtonKind::DeleteMessage => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    request.add(RequestKind::DeleteMessage(
                        cx.bot.delete_message(message.chat_id(), message.id),
                    ));
                }
                request
            }
            ButtonKind::ShowPlan { plan } => Self::edit_callback_text(
                &cx,
                format!("Plan:\n(Click to see details)"),
                Some(
                    Keyboard::new()
                        .buttons(poll_plan_buttons(plan.clone()))
                        .save(&state)
                        .inline_keyboard(),
                ),
            ),
            ButtonKind::RerollPlan { plan } => {
                let mut request = RequestResult::default();
                let meals = state.read().get_saved_meals();
                let meal_plan = Plan::gen(meals, plan.days);
                if let Some(message) = &cx.update.message {
                    state
                        .write()
                        .save_plan(message.chat_id(), meal_plan.clone());

                    let keyboard = Keyboard::new()
                        .buttons(poll_plan_buttons(meal_plan.clone()))
                        .save(&state);
                    request
                        .add(RequestKind::DeleteMessage(
                            cx.bot.delete_message(message.chat_id(), message.id),
                        ))
                        .add(RequestKind::Poll(
                            cx.bot
                                .send_poll(
                                    message.chat_id(),
                                    format!("Plan:\n(Click to see details)"),
                                    meal_plan.answers(),
                                )
                                .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                    keyboard.inline_keyboard(),
                                )),
                            PollKind::Plan { plan: meal_plan },
                            keyboard.id,
                        ));
                }
                request
            }
            ButtonKind::ClearVotes { plan } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let keyboard = Keyboard::new()
                        .buttons(poll_plan_buttons(plan.clone()))
                        .save(&state);
                    request
                        .add(RequestKind::DeleteMessage(
                            cx.bot.delete_message(message.chat_id(), message.id),
                        ))
                        .add(RequestKind::Poll(
                            cx.bot
                                .send_poll(
                                    message.chat_id(),
                                    format!("Plan:\n(Click to see details)"),
                                    plan.answers(),
                                )
                                .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                    keyboard.inline_keyboard(),
                                )),
                            PollKind::Plan { plan: plan.clone() },
                            keyboard.id,
                        ));
                }
                request
            }
            ButtonKind::DisplayListMeal { meal } => {
                let keyboard = Keyboard::new()
                    .buttons(vec![vec![
                        Button::new("Back".to_string(), ButtonKind::ShowList),
                        Button::new("Exit".to_string(), ButtonKind::DeleteMessage),
                    ]])
                    .save(state)
                    .inline_keyboard();
                Self::edit_callback_text(&cx, format!("{}", meal), Some(keyboard))
            }
            ButtonKind::ShowList => {
                let meal_btns: Vec<Vec<Button>> = state
                    .read()
                    .get_saved_meals()
                    .iter()
                    .map(|meal| {
                        vec![Button::new(
                            meal.name.clone(),
                            ButtonKind::DisplayListMeal { meal: meal.clone() },
                        )]
                    })
                    .collect();
                Self::edit_callback_text(
                    &cx,
                    format!("List:"),
                    Some(
                        Keyboard::new()
                            .buttons(meal_btns)
                            .save(&state)
                            .inline_keyboard(),
                    ),
                )
            }
            ButtonKind::CommandButton { command } => command.execute(
                state,
                &UpdateWithCx {
                    bot: cx.bot.clone(),
                    update: cx.update.message.as_ref().cloned().unwrap(),
                },
            ),
            ButtonKind::PollRating { meal } => {
                let mut result = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let answers: Vec<String> = (1..=5)
                        .into_iter()
                        .map(|r| "⭐".repeat(r as usize))
                        .collect();
                    result.add(RequestKind::EditMessage(cx.bot.edit_message_text(
                        message.chat_id(),
                        message.id,
                        format!("{}\n\nVoting...", meal),
                    )));
                    let keyboard = Keyboard::new()
                        .buttons(vec![vec![Button::new(
                            "Cancel".to_uppercase(),
                            ButtonKind::CancelPollRating {
                                meal_id: meal.id.clone(),
                            },
                        )]])
                        .save(state);
                    result.add(RequestKind::Poll(
                        cx.bot
                            .send_poll(
                                ChatId::Id(message.chat_id()),
                                format!("Rate meal: {}", meal.name.to_uppercase()),
                                answers,
                            )
                            .reply_to_message_id(message.id)
                            .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                keyboard.inline_keyboard(),
                            )),
                        PollKind::Meal {
                            meal_id: meal.id.clone(),
                            reply_message_id: message.id,
                        },
                        keyboard.id,
                    ));
                }
                result
            }
            ButtonKind::SavePollRating { meal_id } => {
                let mut result = RequestResult::default();
                if let Some((_, poll)) =
                    state
                        .read()
                        .polls()
                        .iter()
                        .find(|(_, p)| match &p.poll_kind {
                            PollKind::Meal { meal_id: id, .. } => id == meal_id,
                            _ => false,
                        })
                {
                    result.add(RequestKind::StopPoll(
                        cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                    ));
                }
                result
            }
            ButtonKind::CancelPollRating { meal_id } => {
                let mut result = RequestResult::default();
                if let Some((_, mut poll)) =
                    state
                        .write()
                        .polls_mut()
                        .iter_mut()
                        .find(|(_, p)| match &p.poll_kind {
                            PollKind::Meal { meal_id: id, .. } => id == meal_id,
                            _ => false,
                        })
                {
                    poll.is_canceled = true;
                    result.add(RequestKind::StopPoll(
                        cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                    ));
                }
                result
            }
        }
    }
    pub fn execute(&self, state: &StateLock, cx: &ContextCallback) -> RequestResult {
        Self::run(self, state, cx)
    }
}

pub fn rate_meal_button_row(rating: u8, meal_id: &String) -> Vec<Button> {
    (1..=5)
        .into_iter()
        .map(|r| {
            Button::new(
                if r <= rating { "⭐" } else { "⚫" }.to_string(),
                ButtonKind::RateMeal {
                    meal_id: meal_id.clone(),
                    rating: r,
                },
            )
        })
        .collect()
}

pub fn save_meal_button_row(meal_id: &String) -> Vec<Button> {
    let save_button = Button::new(
        "Save Meal".to_uppercase(),
        ButtonKind::SaveMeal {
            meal_id: meal_id.clone(),
        },
    );
    let cancel_button = Button::new("Cancel".to_uppercase(), ButtonKind::DeleteMessage);
    vec![save_button, cancel_button]
}

pub fn save_poll_button_row(meal: &Meal) -> Vec<Button> {
    let save_button = Button::new(
        "Save Meal".to_uppercase(),
        ButtonKind::SavePollRating {
            meal_id: meal.id.clone(),
        },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelPollRating {
            meal_id: meal.id.clone(),
        },
    );
    vec![save_button, cancel_button]
}

pub fn poll_plan_buttons(plan: Plan) -> Vec<Vec<Button>> {
    let meal_info = plan.buttons();
    vec![
        meal_info.concat(),
        vec![
            Button::new(
                "Reroll".to_string(),
                ButtonKind::RerollPlan { plan: plan.clone() },
            ),
            Button::new("Clear".to_string(), ButtonKind::ClearVotes { plan }),
            Button::new("Exit".to_string(), ButtonKind::DeleteMessage),
        ],
    ]
}
