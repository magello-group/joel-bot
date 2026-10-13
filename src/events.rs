use std::sync::Arc;

use crate::client::SlackClient;
use crate::config::Configuration;
use crate::last_day::{next_time_report_day, today};
use chrono::NaiveDate;
use serde::Deserialize;

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
    pub challenge: String,
}

#[derive(Deserialize)]
pub struct EventRequest {
    // TODO: Verify the request signature before handling events.
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
    slack_client: Arc<SlackClient>,
    config: Arc<Configuration>,
}

impl SlackState {
    #[must_use]
    pub fn new(slack_client: Arc<SlackClient>, config: Arc<Configuration>) -> Self {
        SlackState {
            slack_client,
            config,
        }
    }

    #[must_use]
    pub fn handle_request(&self, request: SlackRequest) -> String {
        match request {
            SlackRequest::Challenge(request) => request.challenge,
            SlackRequest::Event(request) => {
                match request.event {
                    Event::AppMentionEvent(event) => self.handle_mention_event(event),
                }
                String::new()
            }
        }
    }

    /// Answers in the background so Slack gets its response within 3 seconds
    /// and doesn't retry the event.
    fn handle_mention_event(&self, event: AppMentionEvent) {
        let client = Arc::clone(&self.slack_client);
        let config = Arc::clone(&self.config);

        tokio::spawn(async move {
            let message = mention_response(&config, &event).await;
            if let Err(error) = client.post_message(&event.channel, &message).await {
                eprintln!("couldn't answer mention: {error:#}");
            }
        });
    }
}

/// The answer to when you need to time report.
#[must_use]
pub fn time_report_message(last_workday: &NaiveDate) -> String {
    format!(
        "Okej, jag har kikat i kalendern och det är först *{last_workday}* som du behöver tidrapportera!"
    )
}

pub const IT_IS_TODAY: &str = "... det är ju idag!";

async fn mention_response(config: &Configuration, event: &AppMentionEvent) -> String {
    // The first word is the mention of the bot itself
    let words: Vec<&str> = event.text.split_whitespace().skip(1).collect();

    match words.first() {
        None => config.get_introduction(),
        Some(&"tid") => {
            let today = today();
            match next_time_report_day(&today).await {
                // Unlike /time-report, which sends the "calculations" as separate
                // delayed messages with random wording, a mention is answered in
                // one message, so the dramatic pause is written out.
                Ok(last_workday) if last_workday == today => {
                    let answer = time_report_message(&last_workday);
                    format!("{answer}\n\n... vänta\n... beräknar\n{IT_IS_TODAY}")
                }
                Ok(last_workday) => time_report_message(&last_workday),
                Err(error) => {
                    eprintln!("couldn't find the next time report day: {error:#}");
                    String::from(
                        "Herregud någonting gick skitfel! Jag kanske behöver uppdatera min firmware :joel:. Kan någon snälla kolla loggen i Azure?",
                    )
                }
            }
        }
        Some(&"pricing") => String::from(
            "För den nätta kostnaden av 114,805 kr per månad eller 15,8 öre per timme kan du hosta din egen joel-bot! :joel:",
        ),
        Some(&"skribenter") => config.get_authors(),
        Some(_) => {
            let user = &event.user;
            let question = words.join(" ");
            format!(
                "Är du skön eller <@{user}>? Tror du att _jag_ vet något om *{question}*? :joel:"
            )
        }
    }
}
