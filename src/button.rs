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
        meal: Meal,
        plan: Plan,
    },
    DisplayListMeal {
        meal: Meal,
    },
    ShowList,
    ShowPlan {
        plan: Plan,
    },
    RerollPlan {
        plan: Plan,
    },
    ClearVotes {
        plan: Plan,
    },
    SaveMeal {
        meal: Meal,
    },
    RateMeal {
        meal: Meal,
        rating: u8,
    },
    RemoveMeal {
        meal: Meal,
    },
    DeleteMeal {
        meal: Meal,
    },
    PollRating {
        meal: Meal,
    },
    SavePollRating {
        meal: Meal,
    },
    CancelPollRating {
        meal: Meal,
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
            ButtonKind::SaveMeal { meal } => {
                let meal_opt = state.write().find_meal(meal.id.to_string()).cloned();
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
            ButtonKind::RemoveMeal { meal } => {
                state.write().remove_meal(&meal);
                Self::run(&ButtonKind::DeleteMessage, state, cx)
            }
            ButtonKind::RateMeal { meal, rating } => {
                state.write().rate_meal(meal.clone(), rating.clone());
                Self::edit_callback_text(
                    &cx,
                    format!("{}\n\nChange rating or save your meal!", meal.clone()),
                    Some(
                        Keyboard::new(chat_id)
                            .buttons(vec![
                                rate_meal_button_row(*rating, &meal),
                                save_meal_button_row(&meal),
                            ])
                            .save(state)
                            .inline_keyboard(),
                    ),
                )
            }
            ButtonKind::DeleteMeal { meal } => {
                state.write().remove_meal(meal);
                Self::edit_callback_text(&cx, format!("{}\n\nRemoved!", meal,), None)
            }
            ButtonKind::DisplayPlanMeal { meal, .. } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
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
                    Keyboard::new(chat_id)
                        .buttons(poll_plan_buttons(plan.clone()))
                        .save(&state)
                        .inline_keyboard(),
                ),
            ),
            ButtonKind::RerollPlan { plan } => {
                let mut request = RequestResult::default();
                if let Some(message) = &cx.update.message {
                    let meals = state.read().get_meals(chat_id);
                    let meal_plan = Plan::gen(chat_id, meals, plan.days);
                    state.write().remove_plan(plan.chat_id);
                    state.write().add_plan(meal_plan.clone());
                    let keyboard = Keyboard::new(chat_id)
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
                    let keyboard = Keyboard::new(chat_id)
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
                let keyboard = Keyboard::new(chat_id)
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
                    .get_meals(chat_id)
                    .iter()
                    .map(|meal| {
                        vec![Button::new(
                            meal.name.clone(),
                            ButtonKind::DisplayListMeal { meal: meal.clone() },
                        )]
                    })
                    .collect();
                if meal_btns.len() > 0 {
                    Self::edit_callback_text(
                        &cx,
                        format!("List:"),
                        Some(
                            Keyboard::new(chat_id)
                                .buttons(meal_btns)
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
                    let keyboard = Keyboard::new(chat_id)
                        .buttons(vec![vec![Button::new(
                            "Cancel".to_uppercase(),
                            ButtonKind::CancelPollRating { meal: meal.clone() },
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
            ButtonKind::SavePollRating { meal } => {
                let mut result = RequestResult::default();
                if let Some(poll) = state.write().find_poll_by_meal_id(meal.id.clone()).cloned() {
                    result.add(RequestKind::StopPoll(
                        cx.bot.stop_poll(poll.chat_id.clone(), poll.message_id),
                    ));
                }
                result
            }
            ButtonKind::CancelPollRating { meal } => {
                let mut result = RequestResult::default();
                if let Some(mut poll) = state.write().find_poll_by_meal_id(meal.id.clone()) {
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

pub fn rate_meal_button_row(rating: u8, meal: &Meal) -> Vec<Button> {
    (1..=5)
        .into_iter()
        .map(|r| {
            Button::new(
                if r <= rating { "⭐" } else { "⚫" }.to_string(),
                ButtonKind::RateMeal {
                    meal: meal.clone(),
                    rating: r,
                },
            )
        })
        .collect()
}

pub fn save_meal_button_row(meal: &Meal) -> Vec<Button> {
    vec![
        Button::new("Ok".to_uppercase(), ButtonKind::DeleteMessage),
        Button::new(
            "Remove".to_uppercase(),
            ButtonKind::RemoveMeal { meal: meal.clone() },
        ),
    ]
}

pub fn save_poll_button_row(meal: &Meal) -> Vec<Button> {
    let save_button = Button::new(
        "Save Meal".to_uppercase(),
        ButtonKind::SavePollRating { meal: meal.clone() },
    );
    let cancel_button = Button::new(
        "Cancel".to_uppercase(),
        ButtonKind::CancelPollRating { meal: meal.clone() },
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
