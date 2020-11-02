use teloxide::requests::*;
use teloxide::types::*;

use crate::poll::{Poll, PollBuildStepOne};
use crate::StateLock;
use crate::state::HasId;

#[derive(Clone, Debug)]
pub enum RequestKind {
    Message(SendMessage, bool),
    Photo(SendPhoto),
    EditMessage(EditMessageText),
    EditInlineMessage(EditInlineMessageText),
    // EditMedia(EditMessageMedia),
    // EditInlineMedia(EditInlineMessageMedia),
    Poll(SendPoll, PollBuildStepOne),
    StopPoll(StopPoll, Option<Poll>),
    DeleteMessage(DeleteMessage),
    EditReplyMarkup(EditMessageReplyMarkup),
    CallbackAnswer(AnswerCallbackQuery),
    EditCaption(EditMessageCaption),
    Pin(PinChatMessage),
}

#[derive(Clone, Debug)]
pub struct RequestResult {
    pub requests: Vec<RequestKind>,
}

impl Default for RequestResult {
    fn default() -> Self {
        Self { requests: vec![] }
    }
}

impl RequestResult {
    pub fn add(&mut self, request: RequestKind) -> &mut Self {
        self.requests.push(request);
        self
    }

    pub fn message(&mut self, message: SendMessage) -> &mut Self {
        self.requests.push(RequestKind::Message(message, false));
        self
    }

    pub async fn send(&self, state: &StateLock) {
        for request in &self.requests {
            match request {
                RequestKind::Message(send_request, notify) => {
                    match send_request
                        .clone()
                        .disable_notification(!notify)
                        .send()
                        .await
                    {
                        Ok(_) => log::info!("Send Message"),
                        Err(err) => log::warn!("Send Message: {}", err),
                    }
                }
                RequestKind::DeleteMessage(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Delete Message"),
                    Err(err) => log::warn!("Delete Message: {}", err),
                },
                RequestKind::Photo(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Send Photo"),
                    Err(err) => log::warn!("Send Photo: {}", err),
                },
                RequestKind::EditMessage(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Message"),
                    Err(err) => log::warn!("Edit Message: {}", err),
                },
                RequestKind::EditReplyMarkup(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Reply Markup"),
                    Err(err) => log::warn!("Edit Reply Markup: {}", err),
                },
                RequestKind::EditInlineMessage(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Inline Message"),
                    Err(err) => log::warn!("Edit Inline Message: {}", err),
                },
                // RequestKind::EditMedia(send_request) => match send_request.send().await {
                //     Ok(_) => log::info!("Edit Media"),
                //     Err(err) => log::warn!("Edit Media: {}", err),
                // },
                // RequestKind::EditInlineMedia(send_request) => match send_request.send().await {
                //     Ok(_) => log::info!("Edit Inline Media"),
                //     Err(err) => log::warn!("Edit Inline Media: {}", err),
                // },
                RequestKind::EditCaption(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Caption"),
                    Err(err) => log::warn!("Edit Caption: {}", err),
                },
                RequestKind::CallbackAnswer(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Callback Answer"),
                    Err(err) => log::warn!("Callback Answer: {}", err),
                },
                RequestKind::Pin(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Pin Message"),
                    Err(err) => log::warn!("Pin Message: {}", err),
                },
                RequestKind::Poll(send_request, poll_builder) => match send_request.send().await {
                    Ok(message) => match message.clone() {
                        Message {
                            kind:
                                MessageKind::Common(MessageCommon {
                                    media_kind: MediaKind::Poll(MediaPoll { poll, .. }),
                                    ..
                                }),
                            id: message_id,
                            ..
                        } => {
                            let poll_id = poll.id;
                            poll_builder.finalize(poll_id, message_id).save(&state);
                            log::info!("Send Poll",);
                        }
                        _ => log::warn!("No Poll found in Message: {:?}", message),
                    },
                    Err(err) => log::warn!("Send Poll: {}", err),
                },
                RequestKind::StopPoll(send_request, poll) => match send_request.send().await {
                    Ok(_) => {
                        if let Some(poll) = poll {
                            match state.write().remove(&poll.id) {
                                Ok(_) => log::debug!("Remove poll"),
                                Err(_) => log::warn!("Error removing poll"),
                            }
                        }
                        log::info!("Stopping Poll")
                    }
                    Err(err) => log::warn!("Error Stop Poll: {}", err),
                },
            }
        }
        // log::debug!("KEYBS: {:?}", state.read().tg.keyboards.len());
        // log::debug!("Chat: {:?}", state.read().chats);
        // log::debug!("MEALS: {:?}", state.read().tg.meals.len());
        // log::debug!("PLANS: {:?}", state.read().tg.plans.len());
        // state.write().save();
    }
}
