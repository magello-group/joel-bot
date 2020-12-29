use serde::Deserialize;
use std::sync::atomic::{AtomicPtr, Ordering};
use crate::client::SlackClient;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum SlackRequest {
    #[serde(rename = "url_verification")]
    Challenge(ChallengeRequest),
    #[serde(rename = "event_callback")]
    Event(EventRequest),
}

#[derive(Deserialize)]
pub struct ChallengeRequest {
    // TODO: Add if needed
    token: String,
    pub challenge: String,
}

#[derive(Deserialize)]
pub struct EventRequest {
    // TODO: Add to use when we get request
    token: String,
    pub event: Event,
}

#[derive(Deserialize, Debug)]
pub struct AppMentionEvent {
    pub user: String,
    pub text: String,
    pub channel: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "app_mention")]
    AppMentionEvent(AppMentionEvent)
}

pub struct SlackEvents {
    token: AtomicPtr<String>,
    slack_client: SlackClient,
    app_mention_event_handler: fn(&SlackClient, AppMentionEvent) -> String,
}

impl SlackEvents {
    pub fn new() -> SlackEvents {
        SlackEvents {
            app_mention_event_handler: |_s, _e| String::new(), // Default func
            slack_client: SlackClient::new().unwrap(),
            token: AtomicPtr::new(&mut String::new()),
        }
    }

    pub fn set_mention_event_handler(&mut self, func: fn(&SlackClient, AppMentionEvent) -> String) {
        self.app_mention_event_handler = func
    }

    fn handle_challenge_request(&self, request: ChallengeRequest) -> String {
        self.token.store(&mut String::from(request.token), Ordering::Relaxed);

        request.challenge
    }

    fn verify_event_then_call(&self, req: EventRequest) -> String {
        // TODO: Verify the token then allow the request.

        match req.event {
            Event::AppMentionEvent(event) => (self.app_mention_event_handler)(&self.slack_client, event),
        }
    }

    pub fn handle_request(&self, request: SlackRequest) -> String {
        match request {
            SlackRequest::Challenge(request) => self.handle_challenge_request(request),
            SlackRequest::Event(request) => self.verify_event_then_call(request)
        }
    }
}
