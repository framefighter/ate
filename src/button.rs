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
use crate::poll::{Poll, PollKind};
use crate::request::{RequestKind, RequestResult};
use crate::{ContextCallback, StateLock};

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ButtonKind {
    DisplayPlanMeal {
        meal_id: String,
        plan_id: String,
    },
    DisplayListMeal {
        meal_id: String,
    },
    ShowList,
    ShowPlan {
        plan_id: String,
    },
    RerollPlan {
        plan_id: String,
    },
    ClearVotes {
        plan_id: String,
    },
    RemovePlanPoll {
        plan_id: String,
    },
    SaveMeal {
        meal_id: String,
    },
    RateMeal {
        meal_id: String,
        rating: u8,
    },
    RemoveMeal {
        meal_id: String,
    },
    DeleteMeal {
        meal_id: String,
    },
    PollRating {
        meal_id: String,
    },
    SavePollRating {
        meal_id: String,
        poll_id: String,
    },
    CancelPollRating {
        poll_id: String,
    },
    CommandButton {
        command: Command,
    },
    PinMessage,
    DeleteMessage,
    #[serde(skip_serializing, skip_deserializing)]
    Request {
        request: RequestResult,
    },
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
        let chat_id = match &cx.update.message {
            Some(msg) => msg.chat_id(),
            None => 0,
        };
        match button {
            ButtonKind::Request { request } => request.clone(),
            ButtonKind::SaveMeal { meal_id } => {
                let meal_opt = state.read().find_meal(meal_id);
                match meal_opt {
                    Some(meal) => {
                        state.write().add_meal(meal.clone());
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
            ButtonKind::RemoveMeal { meal_id } => {
                state.write().remove_meal(meal_id);
                Self::run(&ButtonKind::DeleteMessage, state, cx)
            }
            ButtonKind::RateMeal { meal_id, rating } => {
                let meal_opt = state.write().rate_meal(meal_id, rating.clone());
                Self::edit_callback_text(
                    &cx,
                    if let Some(meal) = meal_opt {
                        format!("{}\n\nChange rating or save your meal!", meal.clone())
                    } else {
                        format!("No meal with to rate!")
                    },
                    Some(
                        Keyboard::new(chat_id)
                            .buttons(vec![
                                rate_meal_button_row(*rating, &meal_id),
                                save_meal_button_row(&meal_id),
                            ])
                            .save(state)
                            .inline_keyboard(),
                    ),
                )
            }
            ButtonKind::DeleteMeal { meal_id } => {
                let meal_opt = state.write().remove_meal(meal_id);
                if let Some(meal) = meal_opt {
                    Self::edit_callback_text(&cx, format!("{}\n\nRemoved!", meal), None)
                } else {
                    Self::edit_callback_text(&cx, format!("No meal to delete found!"), None)
                }
            }
            ButtonKind::DisplayPlanMeal { meal_id, plan_id } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let plan_opt = state.read().find_plan(&chat_id);
                    if let Some(plan) = plan_opt {
                        let keyboard = Keyboard::new(chat_id)
                            .buttons(poll_plan_buttons(&plan))
                            .save(&state);
                        request.add(RequestKind::EditReplyMarkup(
                            cx.bot
                                .edit_message_reply_markup(message.chat_id(), message.id)
                                .reply_markup(keyboard.inline_keyboard()),
                        ));
                    }
                    let meal_opt = state.read().find_meal(meal_id);
                    if let Some(meal) = meal_opt {
                        request.message(
                            cx.bot
                                .send_message(message.chat_id(), format!("{}", meal))
                                .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                    Keyboard::new(chat_id)
                                        .buttons(vec![vec![Button::new(
                                            format!("Back"),
                                            ButtonKind::DeleteMessage,
                                        )]])
                                        .save(state)
                                        .inline_keyboard(),
                                )),
                        );
                    }
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
            ButtonKind::ShowPlan { plan_id } => {
                let plan_opt = state.read().find_plan(&chat_id);
                if let Some(plan) = plan_opt {
                    Self::edit_callback_text(
                        &cx,
                        format!("Plan:\n(Click to vote or use buttons to get meal info)"),
                        Some(
                            Keyboard::new(chat_id)
                                .buttons(poll_plan_buttons(&plan))
                                .save(&state)
                                .inline_keyboard(),
                        ),
                    )
                } else {
                    Self::edit_callback_text(
                        &cx,
                        format!(
                            "No Plan for this chat exists.\n(create a new plan with /plan <days>)"
                        ),
                        None,
                    )
                }
            }
            ButtonKind::RerollPlan { plan_id } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let plan_opt = state.read().find_plan(&chat_id);
                    if let Some(plan) = plan_opt {
                        let meals = state.read().get_meals(chat_id);
                        let meal_plan = Plan::gen(chat_id, meals, plan.days);
                        state.write().remove_plan(&plan.chat_id);
                        state.write().add_plan(meal_plan.clone());
                        let poll_opt = state.read().find_poll_by_plan_id(&plan.id);
                        if let Some(poll) = poll_opt {
                            request.add(RequestKind::StopPoll(
                                cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                                Some(poll),
                            ));
                        }

                        let mut keyboard = Keyboard::new(chat_id);
                        let keyboard_id = keyboard.id.clone();
                        let poll_kind = PollKind::Plan { plan: plan.clone() };
                        let poll_builder = Poll::build(chat_id, poll_kind.clone(), keyboard_id);
                        keyboard = keyboard.buttons(poll_plan_buttons(&plan)).save(&state);
                        request
                            .add(RequestKind::DeleteMessage(
                                cx.bot.delete_message(message.chat_id(), message.id),
                            ))
                            .add(RequestKind::Poll(
                                cx.bot
                                    .send_poll(
                                        message.chat_id(),
                                        format!(
                                        "Plan:\n(Click to vote or use buttons to get meal info)"
                                    ),
                                        meal_plan.answers(),
                                    )
                                    .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                        keyboard.inline_keyboard(),
                                    )),
                                poll_builder,
                            ));
                    }
                }
                request
            }
            ButtonKind::ClearVotes { plan_id } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let poll_opt = state.read().find_poll_by_plan_id(&plan_id);
                    if let Some(poll) = poll_opt {
                        request.add(RequestKind::StopPoll(
                            cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                            Some(poll),
                        ));
                    }
                    let plan_opt = state.read().find_plan(&chat_id);
                    if let Some(plan) = plan_opt {
                        let mut keyboard = Keyboard::new(chat_id);
                        let keyboard_id = keyboard.id.clone();
                        let poll_kind = PollKind::Plan { plan: plan.clone() };
                        let poll_builder = Poll::build(chat_id, poll_kind.clone(), keyboard_id);
                        keyboard = keyboard.buttons(poll_plan_buttons(&plan)).save(&state);
                        request
                            .add(RequestKind::DeleteMessage(
                                cx.bot.delete_message(message.chat_id(), message.id),
                            ))
                            .add(RequestKind::Poll(
                                cx.bot
                                    .send_poll(
                                        message.chat_id(),
                                        format!(
                                        "Plan:\n(Click to vote or use buttons to get meal info)"
                                    ),
                                        plan.answers(),
                                    )
                                    .reply_markup(ReplyMarkup::InlineKeyboardMarkup(
                                        keyboard.inline_keyboard(),
                                    )),
                                poll_builder,
                            ));
                    }
                }
                request
            }
            ButtonKind::RemovePlanPoll { plan_id } => {
                let mut request = RequestResult::default();
                let poll_opt = state.read().find_poll_by_plan_id(&plan_id);
                if let Some(poll) = poll_opt {
                    request
                        .add(RequestKind::StopPoll(
                            cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                            Some(poll.clone()),
                        ))
                        .add(RequestKind::DeleteMessage(
                            cx.bot.delete_message(poll.chat_id.clone(), poll.message_id),
                        ));
                }
                request
            }
            ButtonKind::DisplayListMeal { meal_id } => {
                let keyboard = Keyboard::new(chat_id)
                    .buttons(vec![vec![
                        Button::new("Back".to_string(), ButtonKind::ShowList),
                        Button::new("Exit".to_string(), ButtonKind::DeleteMessage),
                    ]])
                    .save(state)
                    .inline_keyboard();
                let meal_opt = state.read().find_meal(meal_id);
                if let Some(meal) = meal_opt {
                    Self::edit_callback_text(&cx, format!("{}", meal), Some(keyboard))
                } else {
                    Self::edit_callback_text(&cx, format!("No meal found!"), Some(keyboard))
                }
            }
            ButtonKind::ShowList => {
                let meal_buttons = state.read().meal_buttons(chat_id);
                if meal_buttons.len() > 0 {
                    Self::edit_callback_text(
                        &cx,
                        format!("List:\n(Click to get more information)"),
                        Some(
                            Keyboard::new(chat_id)
                                .buttons(meal_buttons)
                                .save(&state)
                                .inline_keyboard(),
                        ),
                    )
                } else {
                    Self::edit_callback_text(
                        &cx,
                        format!("No meals saved!\n(save new meals with /new <meal name>)"),
                        None,
                    )
                }
            }
            ButtonKind::CommandButton { command } => command.execute(
                state,
                &UpdateWithCx {
                    bot: cx.bot.clone(),
                    update: cx.update.message.as_ref().cloned().unwrap(),
                },
            ),
            ButtonKind::PollRating { meal_id } => {
                let mut result = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let answers: Vec<String> = (1..=5)
                        .into_iter()
                        .map(|r| "⭐".repeat(r as usize))
                        .collect();
                    let meal_opt = state.read().find_meal(meal_id);
                    if let Some(meal) = meal_opt {
                        result.add(RequestKind::EditMessage(cx.bot.edit_message_text(
                            message.chat_id(),
                            message.id,
                            format!("{}\n\nVoting...", meal),
                        )));
                        let poll_kind = PollKind::Meal {
                            meal_id: meal.id.clone(),
                            reply_message_id: message.id,
                        };
                        let mut keyboard = Keyboard::new(chat_id);
                        let keyboard_id = keyboard.id.clone();
                        let poll_builder = Poll::build(chat_id, poll_kind.clone(), keyboard_id);
                        keyboard = keyboard
                            .buttons(vec![vec![Button::new(
                                "Cancel".to_uppercase(),
                                ButtonKind::CancelPollRating {
                                    poll_id: poll_builder.id.clone(),
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
                            poll_builder,
                        ));
                    }
                }
                result
            }
            ButtonKind::SavePollRating { poll_id, .. } => {
                let mut result = RequestResult::default();
                match state.read().find_poll(&poll_id) {
                    Some(
                        poll
                        @
                        Poll {
                            poll_kind: PollKind::Meal { .. },
                            ..
                        },
                    ) => {
                        result.add(RequestKind::StopPoll(
                            cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                            None,
                        ));
                    }
                    _ => {}
                }
                result
            }
            ButtonKind::CancelPollRating { poll_id } => {
                let mut result = RequestResult::default();
                state
                    .write()
                    .poll_entry(poll_id.to_string())
                    .and_modify(|poll| {
                        poll.is_canceled = true;
                        result.add(RequestKind::StopPoll(
                            cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                            Some(poll.clone()),
                        ));
                    });
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
    vec![
        Button::new("Ok".to_uppercase(), ButtonKind::DeleteMessage),
        Button::new(
            "Remove".to_uppercase(),
            ButtonKind::RemoveMeal {
                meal_id: meal_id.clone(),
            },
        ),
    ]
}

pub fn save_poll_button_row(meal_id: &String, poll_id: &String) -> Vec<Button> {
    let save_button = Button::new(
        "Save".to_uppercase(),
        ButtonKind::SavePollRating {
            meal_id: meal_id.clone(),
            poll_id: poll_id.clone(),
        },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelPollRating {
            poll_id: poll_id.clone(),
        },
    );
    vec![save_button, cancel_button]
}

pub fn poll_plan_buttons(plan: &Plan) -> Vec<Vec<Button>> {
    let meal_info = plan.buttons();
    vec![
        meal_info,
        vec![vec![
            Button::new(
                "Reroll".to_string(),
                ButtonKind::RerollPlan {
                    plan_id: plan.id.clone(),
                },
            ),
            Button::new(
                "Clear".to_string(),
                ButtonKind::ClearVotes {
                    plan_id: plan.id.clone(),
                },
            ),
            Button::new(
                "Exit".to_string(),
                ButtonKind::RemovePlanPoll {
                    plan_id: plan.id.clone(),
                },
            ),
        ]],
    ]
    .concat()
}
