use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use teloxide::dispatching::UpdateWithCx;
use teloxide::types::Poll as TgPoll;

use crate::button;
use crate::button::{Button, ButtonKind};
use crate::keyboard::Keyboard;
use crate::meal::Meal;

use crate::request::{RequestKind, RequestResult};
use crate::state::HasId;
use crate::StateLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PollKind {
    Meal {
        meal_id: String,
        reply_message_id: i32,
    },
    Plan {
        plan_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollBuildStepOne {
    pub id: String,
    pub chat_id: i64,
    pub poll_kind: PollKind,
    pub is_canceled: bool,
    pub keyboard_id: String,
}

impl PollBuildStepOne {
    pub fn finalize(&self, poll_id: String, message_id: i32) -> Poll {
        Poll {
            id: self.id.clone(),
            poll_id,
            chat_id: self.chat_id,
            message_id,
            poll_kind: self.poll_kind.clone(),
            keyboard_id: self.keyboard_id.clone(),
            is_canceled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub id: String,
    pub poll_id: String,
    pub chat_id: i64,
    pub message_id: i32,
    pub poll_kind: PollKind,
    pub is_canceled: bool,
    pub keyboard_id: String,
}

impl HasId for Poll {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn chat_id(&self) -> i64 {
        self.chat_id
    }
    fn save(&self, state: &StateLock) -> Self {
        match state.write().add(self) {
            Ok(_) => log::debug!("Saved poll"),
            Err(_) => log::warn!("Error saving poll"),
        }
        self.clone()
    }
}

impl Poll {
    pub fn new(
        poll_id: String,
        chat_id: i64,
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

    pub fn build(chat_id: i64, poll_kind: PollKind, keyboard_id: String) -> PollBuildStepOne {
        PollBuildStepOne {
            id: nanoid!(),
            chat_id,
            poll_kind,
            keyboard_id,
            is_canceled: false,
        }
    }

    pub fn cancel(&mut self) -> &mut Self {
        self.is_canceled = true;
        self
    }

    pub fn handle_votes(&self, state: &StateLock, cx: &UpdateWithCx<TgPoll>) -> RequestResult {
        match &self.poll_kind {
            PollKind::Meal {
                meal_id,
                reply_message_id,
            } => {
                let meal_opt: Option<Meal> = state.read().get(meal_id);
                match meal_opt {
                    None => {
                        log::warn!("No meal with id {} found for poll: {:?}", meal_id, self);
                        RequestResult::default()
                            .add(RequestKind::StopPoll(
                                cx.bot.stop_poll(self.chat_id.clone(), self.message_id),
                                Some(self.clone()),
                            ))
                            .clone()
                    }
                    Some(meal) => {
                        let total_votes = cx.update.total_voter_count;
                        if cx.update.is_closed {
                            match state.write().remove(&self.id) {
                                Ok(_) => log::debug!("Removed poll"),
                                Err(_) => log::warn!("Error removing poll"),
                            }
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
                                match state.write().modify(meal_id, |mut meal: Meal| {
                                    meal.rate(Some(
                                        ((avg as u8) + meal.rating.unwrap_or(avg as u8)) / 2,
                                    ))
                                    .clone()
                                }) {
                                    Ok(_) => log::debug!("Modified meal"),
                                    Err(_) => log::warn!("Error modifying meal"),
                                }
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
                                                Keyboard::new(self.chat_id)
                                                    .buttons(vec![
                                                        vec![Button::new(
                                                            "Rate with Poll".into(),
                                                            ButtonKind::PollRating {
                                                                meal_id: meal.id.clone(),
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
                            match state.write().remove(&self.keyboard_id) {
                                Ok(_) => log::debug!("Removed keyboard"),
                                Err(_) => log::warn!("Error removing keyboard"),
                            }
                            log::info!("Poll Vote...",);
                            if total_votes > 0 {
                                let keyboard = Keyboard::new(self.chat_id)
                                    .buttons(vec![button::save_poll_button_row(&meal.id, &self.id)])
                                    .save(&state);
                                let new_poll = Poll::new(
                                    self.poll_id.clone(),
                                    self.chat_id,
                                    self.message_id,
                                    self.poll_kind.clone(),
                                    keyboard.id.clone(),
                                );
                                new_poll.save(state);
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
                                let keyboard = Keyboard::new(self.chat_id)
                                    .buttons(vec![vec![Button::new(
                                        "Cancel Vote".to_uppercase(),
                                        ButtonKind::CancelPollRating {
                                            poll_id: self.id.clone(),
                                        },
                                    )]])
                                    .save(&state);
                                let new_poll = Poll::new(
                                    self.poll_id.clone(),
                                    self.chat_id,
                                    self.message_id,
                                    self.poll_kind.clone(),
                                    keyboard.id.clone(),
                                );
                                new_poll.save(state);
                                // hide show button
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
            PollKind::Plan { .. } => RequestResult::default(),
        }
    }
}
