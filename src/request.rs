use teloxide::requests::*;
use teloxide::types::*;

use crate::meal::Meal;
use crate::poll::Poll;
use crate::StateLock;

pub enum RequestKind {
    Message(SendMessage),
    Photo(SendPhoto),
    EditMessage(EditMessageText),
    EditInlineMessage(EditInlineMessageText),
    EditMedia(EditMessageMedia),
    EditInlineMedia(EditInlineMessageMedia),
    Poll(SendPoll, Meal, i32),
    StopPoll(StopPoll),
}

pub struct RequestResult {
    pub requests: Vec<RequestKind>,
}

impl Default for RequestResult {
    fn default() -> Self {
        Self { requests: vec![] }
    }
}

impl RequestResult {
    pub fn add(&mut self, request: RequestKind) -> &Self {
        self.requests.push(request);
        self
    }

    pub fn message(&mut self, message: SendMessage) -> &Self {
        self.requests.push(RequestKind::Message(message));
        self
    }

    pub async fn send(&self, state: &StateLock) {
        for request in &self.requests {
            match request {
                RequestKind::Message(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Send Message"),
                    Err(err) => log::warn!("Send Message: {}", err),
                },
                RequestKind::Photo(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Send Photo"),
                    Err(err) => log::warn!("Send Photo: {}", err),
                },
                RequestKind::EditMessage(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Message"),
                    Err(err) => log::warn!("Edit Message: {}", err),
                },
                RequestKind::EditInlineMessage(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Inline Message"),
                    Err(err) => log::warn!("Edit Inline Message: {}", err),
                },
                RequestKind::EditMedia(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Media"),
                    Err(err) => log::warn!("Edit Media: {}", err),
                },
                RequestKind::EditInlineMedia(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Edit Inline Media"),
                    Err(err) => log::warn!("Edit Inline Media: {}", err),
                },
                RequestKind::Poll(send_request, meal, reply_message_id) => {
                    match send_request.send().await {
                        Ok(message) => match message.clone() {
                            Message {
                                kind:
                                    MessageKind::Common(MessageCommon {
                                        media_kind: MediaKind::Poll(MediaPoll { poll, .. }),
                                        ..
                                    }),
                                id: message_id,
                                chat:
                                    Chat {
                                        id: chat_id_raw, ..
                                    },
                                ..
                            } => {
                                let poll_id = poll.id;
                                let chat_id = ChatId::Id(chat_id_raw);
                                Poll::new(
                                    poll_id,
                                    chat_id,
                                    message_id,
                                    *reply_message_id,
                                    meal.id.clone(),
                                )
                                .save(&state);
                                log::info!("Send Poll",);
                            }
                            _ => log::warn!("No Poll found in Message: {:?}", message),
                        },
                        Err(err) => log::warn!("Send Poll: {}", err),
                    }
                }
                RequestKind::StopPoll(send_request) => match send_request.send().await {
                    Ok(_) => log::info!("Stop Poll"),
                    Err(err) => log::warn!("Error Stop Poll: {}", err),
                },
            }
        }
    }
}
