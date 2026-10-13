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
    /// Other request types, e.g. `app_rate_limited`, acknowledged so Slack doesn't retry them.
    #[serde(other)]
    Unknown,
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
    /// Missing when the mention was written by a bot or workflow.
    pub user: Option<String>,
    pub text: String,
    pub channel: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "app_mention")]
    AppMentionEvent(AppMentionEvent),
    /// Event types we don't handle, acknowledged so Slack doesn't retry them.
    #[serde(other)]
    Unknown,
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
                    Event::Unknown => {}
                }
                String::new()
            }
            SlackRequest::Unknown => String::new(),
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
    // The command follows the first mention, which can be anywhere in the
    // text. Mentions are skipped, of the bot itself or anyone else.
    let is_mention = |word: &&str| word.starts_with("<@");
    let words: Vec<&str> = event
        .text
        .split_whitespace()
        .skip_while(|word| !is_mention(word))
        .filter(|word| !is_mention(word))
        .collect();

    let command = words.first().map(|word| word.to_lowercase());

    match command.as_deref() {
        None => config.get_introduction(),
        Some("tid") => {
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
        Some("pricing") => String::from(
            "För den nätta kostnaden av 114,805 kr per månad eller 15,8 öre per timme kan du hosta din egen joel-bot! :joel:",
        ),
        Some("skribenter") => config.get_authors(),
        Some(_) => {
            let user = event
                .user
                .as_ref()
                .map(|user| format!(" <@{user}>"))
                .unwrap_or_default();
            let question = words.join(" ");
            format!("Är du skön eller{user}? Tror du att _jag_ vet något om *{question}*? :joel:")
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn mention(text: &str, user: Option<&str>) -> AppMentionEvent {
        AppMentionEvent {
            user: user.map(String::from),
            text: String::from(text),
            channel: String::from("C1"),
        }
    }

    #[test]
    fn parses_mention_from_bot_without_user() {
        let request: SlackRequest = serde_json::from_str(
            r#"{"type":"event_callback","event":{"type":"app_mention","bot_id":"B1","text":"<@U0> tid","channel":"C1"}}"#,
        )
        .unwrap();
        assert!(matches!(
            request,
            SlackRequest::Event(EventRequest {
                event: Event::AppMentionEvent(AppMentionEvent { user: None, .. })
            })
        ));
    }

    #[test]
    fn parses_unknown_request_and_event_types() {
        let request: SlackRequest =
            serde_json::from_str(r#"{"type":"app_rate_limited","minute_rate_limited":1}"#).unwrap();
        assert!(matches!(request, SlackRequest::Unknown));
        let request: SlackRequest = serde_json::from_str(
            r#"{"type":"event_callback","event":{"type":"reaction_added","user":"U1"}}"#,
        )
        .unwrap();
        assert!(matches!(
            request,
            SlackRequest::Event(EventRequest {
                event: Event::Unknown
            })
        ));
    }

    #[tokio::test]
    async fn mention_can_be_anywhere_and_any_case() {
        let config = Configuration::read().unwrap();
        let pricing = mention_response(&config, &mention("<@U0> pricing", Some("U1"))).await;
        assert!(pricing.contains("114,805 kr"), "{pricing}");
        let answer = mention_response(&config, &mention("hej <@U0> PRICING", Some("U1"))).await;
        assert_eq!(answer, pricing);
    }

    #[tokio::test]
    async fn unknown_command_without_user() {
        let config = Configuration::read().unwrap();
        let answer = mention_response(&config, &mention("<@U0> hej", None)).await;
        assert!(answer.starts_with("Är du skön eller? "), "{answer}");
        assert!(answer.contains("*hej*"), "{answer}");
    }
}
