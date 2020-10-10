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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub id: String,
    pub poll_id: String,
    pub chat_id: ChatId,
    pub message_id: i32,
    pub reply_message_id: i32,
    pub meal_id: String,
    pub is_canceled: bool,
    pub keyboard_id: String,
}

impl Poll {
    pub fn new(
        poll_id: String,
        chat_id: ChatId,
        message_id: i32,
        reply_message_id: i32,
        meal_id: String,
        keyboard_id: String,
    ) -> Self {
        Self {
            id: nanoid!(),
            poll_id,
            chat_id,
            message_id,
            reply_message_id,
            meal_id,
            keyboard_id,
            is_canceled: false,
        }
    }

    pub fn save(self, state: &StateLock) -> Self {
        state.write().polls.insert(self.id.clone(), self.clone());
        self
    }

    pub fn handle_votes(
        &mut self,
        state: &StateLock,
        cx: &UpdateWithCx<TgPoll>,
        meal: Meal,
    ) -> RequestResult {
        let total_votes = cx.update.total_voter_count;
        state.write().remove_poll(self.id.clone());
        if cx.update.is_closed {
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
                let avg = votes.iter().fold(0, |sum, vote| sum + vote.0 * vote.1) / total_votes;
                let mut meal = meal.clone();
                meal.rate(Some(((avg as u8) + meal.rating.unwrap_or(avg as u8)) / 2));
                state.write().save_meal(&meal);
                log::info!("Poll closed: {}", meal.name);
                // tell user that meal has been saved with new rating
                RequestResult::default()
                    .add(RequestKind::EditMessage(cx.bot.edit_message_text(
                        self.chat_id.clone(),
                        self.reply_message_id,
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
                                self.reply_message_id,
                                format!("{}\n\nPoll Canceled!", meal),
                            )
                            .reply_markup(
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
                    ))
                    .add(RequestKind::DeleteMessage(
                        cx.bot.delete_message(self.chat_id.clone(), self.message_id),
                    ))
                    .clone()
            }
        } else {
            // poll still in progress
            // remove poll keyboard
            state.write().remove_keyboard(self.keyboard_id.clone());
            log::info!("Poll Vote. {:?}", self);
            if total_votes > 0 {
                let keyboard = Keyboard::new()
                    .buttons(vec![button::save_poll_button_row(meal)])
                    .save(&state);
                self.keyboard_id = keyboard.id.clone();
                // show save button
                RequestResult::default()
                    .add(RequestKind::EditReplyMarkup(
                        cx.bot
                            .edit_message_reply_markup(self.chat_id.clone(), self.message_id)
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
                self.keyboard_id = keyboard.id.clone();
                // hide show button
                RequestResult::default()
                    .add(RequestKind::EditReplyMarkup(
                        cx.bot
                            .edit_message_reply_markup(self.chat_id.clone(), self.message_id)
                            .reply_markup(keyboard.inline_keyboard()),
                    ))
                    .clone()
            }
        }
    }
}
