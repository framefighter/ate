use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use teloxide::dispatching::UpdateWithCx;
use teloxide::types::{ChatId, Poll as TgPoll};

use crate::button;
use crate::button::{Button, ButtonKind};
use crate::keyboard::Keyboard;
use crate::meal::Meal;
use crate::request::{RequestKind, RequestResult};
use crate::StateLock;
use crate::plan::Plan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PollKind {
    Meal {
        meal_id: String,
        reply_message_id: i32,
    },
    Plan {
        plan: Plan,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub id: String,
    pub poll_id: String,
    pub chat_id: ChatId,
    pub message_id: i32,
    pub poll_kind: PollKind,
    pub is_canceled: bool,
    pub keyboard_id: String,
}

impl Poll {
    pub fn new(
        poll_id: String,
        chat_id: ChatId,
        message_id: i32,
        poll_kind: PollKind,
        keyboard_id: String,
    ) -> Self {
        Self {
            id: nanoid!(),
            poll_id,
            chat_id,
            message_id,
            poll_kind,
            keyboard_id,
            is_canceled: false,
        }
    }

    pub fn save(self, state: &StateLock) -> Self {
        state
            .write()
            .polls_mut()
            .insert(self.id.clone(), self.clone());
        self
    }

    pub fn handle_votes(&self, state: &StateLock, cx: &UpdateWithCx<TgPoll>) -> RequestResult {
        match &self.poll_kind {
            PollKind::Meal {
                meal_id,
                reply_message_id,
            } => {
                let meals = state.read().meals().clone();
                let meal_opt = meals.get(meal_id).clone();
                match meal_opt {
                    None => {
                        state.write().polls_mut().remove(&self.id.clone());
                        log::warn!("No meal with id {} found for poll: {:?}", meal_id, self);
                        RequestResult::default()
                            .add(RequestKind::StopPoll(
                                cx.bot.stop_poll(self.chat_id.clone(), self.message_id),
                            ))
                            .clone()
                    }
                    Some(meal) => {
                        let total_votes = cx.update.total_voter_count;
                        if cx.update.is_closed {
                            state.write().polls_mut().remove(&self.id.clone());
                            if total_votes > 0 && !self.is_canceled {
                                // someone voted and poll closed successfully ->
                                //              update meal and save meal and poll
                                let votes: Vec<(i32, i32)> = cx
                                    .update
                                    .options
                                    .iter()
                                    .enumerate()
                                    .map(|(i, po)| ((i + 1) as i32, po.voter_count))
                                    .collect();
                                let avg = votes.iter().fold(0, |sum, vote| sum + vote.0 * vote.1)
                                    / total_votes;
                                let mut meal = meal.clone();
                                meal.rate(Some(
                                    ((avg as u8) + meal.rating.unwrap_or(avg as u8)) / 2,
                                ));
                                state.write().save_meal(&meal);
                                state.write().meals_mut().remove(&meal.id);
                                log::info!("Poll closed: {}", meal.name);
                                // tell user that meal has been saved with new rating
                                RequestResult::default()
                                    .add(RequestKind::EditMessage(cx.bot.edit_message_text(
                                        self.chat_id.clone(),
                                        *reply_message_id,
                                        format!("{}\n\nSaved!", meal),
                                    )))
                                    .clone()
                            } else {
                                // nobody voted or vote got canceled -> remove poll
                                log::info!("Poll ended: {}", meal.name);
                                // tell user that vote endet but nobody voted
                                // and remove poll message and show old message again
                                RequestResult::default()
                                    .add(RequestKind::EditMessage(
                                        cx.bot
                                            .edit_message_text(
                                                self.chat_id.clone(),
                                                *reply_message_id,
                                                format!("{}\n\nPoll Canceled!", meal),
                                            )
                                            .reply_markup(
                                                Keyboard::new()
                                                    .buttons(vec![
                                                        vec![Button::new(
                                                            "Rate with Poll".into(),
                                                            ButtonKind::PollRating {
                                                                meal: meal.clone(),
                                                            },
                                                        )],
                                                        button::save_meal_button_row(&meal.id),
                                                    ])
                                                    .save(&state)
                                                    .inline_keyboard(),
                                            ),
                                    ))
                                    .add(RequestKind::DeleteMessage(
                                        cx.bot
                                            .delete_message(self.chat_id.clone(), self.message_id),
                                    ))
                                    .clone()
                            }
                        } else {
                            // poll still in progress
                            // remove poll keyboard
                            state
                                .write()
                                .keyboards_mut()
                                .remove(&self.keyboard_id.clone());
                            log::info!("Poll Vote...",);
                            if total_votes > 0 {
                                let keyboard = Keyboard::new()
                                    .buttons(vec![button::save_poll_button_row(&meal)])
                                    .save(&state);
                                state.write().polls_mut().iter_mut().for_each(|(_, poll)| {
                                    if poll.id == self.id {
                                        poll.keyboard_id = keyboard.id.clone()
                                    }
                                });
                                // show save button
                                RequestResult::default()
                                    .add(RequestKind::EditReplyMarkup(
                                        cx.bot
                                            .edit_message_reply_markup(
                                                self.chat_id.clone(),
                                                self.message_id,
                                            )
                                            .reply_markup(keyboard.inline_keyboard()),
                                    ))
                                    .clone()
                            } else {
                                let keyboard = Keyboard::new()
                                    .buttons(vec![vec![Button::new(
                                        "Cancel Vote".to_uppercase(),
                                        ButtonKind::CancelPollRating {
                                            meal_id: meal.id.clone(),
                                        },
                                    )]])
                                    .save(&state);
                                state.write().polls_mut().iter_mut().for_each(|(_, poll)| {
                                    if poll.id == self.id {
                                        poll.keyboard_id = keyboard.id.clone()
                                    }
                                }); // hide show button
                                RequestResult::default()
                                    .add(RequestKind::EditReplyMarkup(
                                        cx.bot
                                            .edit_message_reply_markup(
                                                self.chat_id.clone(),
                                                self.message_id,
                                            )
                                            .reply_markup(keyboard.inline_keyboard()),
                                    ))
                                    .clone()
                            }
                        }
                    }
                }
            }
            PollKind::Plan { plan } => RequestResult::default(),
        }
    }
}
