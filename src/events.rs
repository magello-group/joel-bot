use crate::client::{SlackClient, SlackClientTrait};
use crate::config::Configuration;
use crate::last_day::get_last_workday;
use chrono::Utc;
use serde::Deserialize;
use std::sync::atomic::{AtomicPtr, Ordering};


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
    #[allow(dead_code)]
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
    AppMentionEvent(AppMentionEvent),
}

pub struct SlackState {
    token: AtomicPtr<String>,
    slack_client: SlackClient,
}

impl SlackState {
    pub fn new() -> Self
    {
        SlackState {
            slack_client: SlackClient::new().unwrap(),
            token: AtomicPtr::new(Box::into_raw(Box::new(String::new()))), // Fixing memory management
        }
    }

    async fn handle_challenge_request(&self, request: ChallengeRequest) -> String {
        self.token
            .store(&mut String::from(request.token), Ordering::Relaxed);
        request.challenge
    }

    async fn verify_event_then_call(&self, req: EventRequest) -> String {
        // TODO: Verify the token then allow the request.

        match req.event {
            Event::AppMentionEvent(event) => {
                SlackState::handle_mention_event(&self.slack_client, event).await
            }
        }
    }

    pub async fn handle_request(&self, request: SlackRequest) -> String {
        match request {
            SlackRequest::Challenge(request) => self.handle_challenge_request(request).await,
            SlackRequest::Event(request) => self.verify_event_then_call(request).await,
        }
    }

    async fn handle_mention_event(
        client: &impl SlackClientTrait,
        event: AppMentionEvent,
    ) -> String {
        let config = Configuration::read().expect("couldn't read configuration when mentioned");
        let mut splits: Vec<&str> = event.text.split(" ").collect();
        splits.drain(0..1);

        let message: String = if splits.len() > 0 {
            match splits[0] {
                "tid" => {
                    let today = Utc::now().naive_utc().date();
                    match get_last_workday(&today).await {
                        Ok(last_workday) => {
                            if last_workday == today {
                                format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!\n\n... vänta\n... beräknar\n... det är ju idag!", last_workday)
                            } else {
                                format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!", last_workday)
                            }
                        }
                        Err(error) => {
                            println!("{}", error);
                            String::from("Herregud någonting gick skitfel! Jag kanske behöver uppdatera min firmware :joel:. Kan någon snälla kolla loggen i Azure?")
                        }
                    }
                }
                "pricing" => {
                    String::from("För den nätta kostnaden av 114,805 kr per månad eller 15,8 öre per timme kan du hosta din egen joel-bot! :joel:")
                }
                "skribenter" => {
                    config.get_authors()
                }
                _command => {
                    format!("Är du skön eller <@{}>? Tror du att _jag_ vet något om *{}*? :joel:", event.user, splits.join(" "))
                }
            }
        } else {
            config.get_introduction()
        };
        client
            .post_message(&event.channel, &message).await
            .unwrap_or_else(|error| println!("{}", error));

        String::new()
    }

}
