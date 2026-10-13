use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::header::RETRY_AFTER;
use reqwest::{Response, StatusCode};
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};

const RATE_LIMIT_ATTEMPTS: u32 = 3;
/// Upper bound on how long to honour a `Retry-After` from Slack.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);

#[derive(Deserialize, Debug)]
pub struct Channel {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Debug, Default)]
struct ResponseMetadata {
    #[serde(default)]
    next_cursor: String,
}

/// Slack answers with HTTP 200 even on errors, the real status is in `ok`.
#[derive(Deserialize, Debug)]
struct SlackStatus {
    ok: bool,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChannelResponse {
    channels: Vec<Channel>,
    #[serde(default)]
    response_metadata: ResponseMetadata,
}

pub struct SlackClient {
    token: String,
}

impl SlackClient {
    pub fn new() -> Result<SlackClient> {
        let token =
            std::env::var("JOEL_BOT_SLACK_TOKEN").context("JOEL_BOT_SLACK_TOKEN is not set")?;
        Ok(SlackClient { token })
    }

    /// Calls a Slack API method, retrying when rate limited.
    async fn call<T: DeserializeOwned>(&self, method: &str, params: &[(&str, &str)]) -> Result<T> {
        let mut attempt = 1;
        loop {
            let response = crate::http::CLIENT
                .post(format!("https://slack.com/api/{method}"))
                .bearer_auth(&self.token)
                .form(params)
                .send()
                .await?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_ATTEMPTS {
                let wait = retry_after(&response)
                    .unwrap_or(Duration::from_secs(5))
                    .min(MAX_RATE_LIMIT_WAIT);
                eprintln!("rate limited by slack on {method}, retrying in {wait:?}");
                tokio::time::sleep(wait).await;
                attempt += 1;
                continue;
            }

            // Prefer Slack's own error from the body over the bare HTTP status
            let status = response.status();
            let body = response.text().await?;
            if !status.is_success() && serde_json::from_str::<SlackStatus>(&body).is_err() {
                bail!("slack {method} returned HTTP {status}");
            }
            return parse_slack_response(method, &body);
        }
    }

    pub async fn get_channel_id_by_name(&self, channel_name: &str) -> Result<Option<String>> {
        Ok(self
            .get_channels()
            .await?
            .into_iter()
            .find(|channel| channel.name == channel_name)
            .map(|channel| channel.id))
    }

    pub async fn get_channels(&self) -> Result<Vec<Channel>> {
        let mut channels = Vec::new();
        let mut cursor = String::new();

        loop {
            let mut params = vec![
                ("types", "private_channel,public_channel"),
                ("exclude_archived", "true"),
                ("limit", "1000"),
            ];
            if !cursor.is_empty() {
                params.push(("cursor", &cursor));
            }
            let response: ChannelResponse = self.call("conversations.list", &params).await?;

            channels.extend(response.channels);
            if response.response_metadata.next_cursor.is_empty() {
                break;
            }
            cursor = response.response_metadata.next_cursor;
        }

        Ok(channels)
    }

    pub async fn post_message(&self, channel_id: &str, message: &str) -> Result<()> {
        self.call::<IgnoredAny>(
            "chat.postMessage",
            &[("channel", channel_id), ("text", message)],
        )
        .await?;
        Ok(())
    }
}

fn retry_after(response: &Response) -> Option<Duration> {
    let seconds = response
        .headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

fn parse_slack_response<T: DeserializeOwned>(method: &str, body: &str) -> Result<T> {
    let status: SlackStatus = serde_json::from_str(body)?;
    if !status.ok {
        let reason = status.error.as_deref().unwrap_or("unknown error");
        bail!("slack {method} failed: {reason}");
    }
    serde_json::from_str(body)
        .with_context(|| format!("slack {method} returned an unexpected response"))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn slack_error_is_an_error() {
        let result: Result<IgnoredAny> = parse_slack_response(
            "chat.postMessage",
            r#"{"ok":false,"error":"not_in_channel"}"#,
        );
        let error = result.unwrap_err().to_string();
        assert!(error.contains("not_in_channel"), "{error}");
    }

    #[test]
    fn channels_without_response_metadata() {
        let response: ChannelResponse = parse_slack_response(
            "conversations.list",
            r#"{"ok":true,"channels":[{"id":"C1","name":"allmant"}]}"#,
        )
        .unwrap();
        assert_eq!(response.channels[0].id, "C1");
        assert_eq!(response.response_metadata.next_cursor, "");
    }

    #[test]
    fn post_message_ok() {
        let result: Result<IgnoredAny> = parse_slack_response(
            "chat.postMessage",
            r#"{"ok":true,"channel":"C1","ts":"1.2","message":{}}"#,
        );
        assert!(result.is_ok());
    }
}
