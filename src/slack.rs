use std::collections::HashMap;
use std::error::Error;

use reqwest::blocking::Client;
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
    // TODO: Add if needed
    // token: String,
    pub challenge: String,
}

#[derive(Deserialize)]
pub struct EventRequest {
    // TODO: Add if needed
    // token: String,
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

#[derive(Deserialize, Debug)]
pub struct Channel {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct ResponseMetadata {
    next_cursor: String
}

#[derive(Deserialize, Debug)]
pub struct ChannelResponse {
    ok: bool,
    channels: Vec<Channel>,
    response_metadata: ResponseMetadata,
}

pub struct SlackClient {
    client: Client,
    token: String,
}

impl SlackClient {
    pub fn new(token: &str) -> SlackClient {
        SlackClient {
            client: Client::new(),
            token: String::from(token),
        }
    }

    pub fn get_channel_id_by_name(&self, channel_name: &str) -> Option<String> {
        match self.get_channels() {
            Ok(channels) => {
                channels.iter().find(|&channel| {
                    channel.name == channel_name
                })
                    .map(|channel| { channel.id.clone() })
            }
            Err(error) => {
                println!("{}", error);
                None
            }
        }
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, Box<dyn Error>> {
        let mut params = HashMap::new();
        params.insert("token", self.token.clone());
        params.insert("types", String::from("private_channel"));
        let mut channels = Vec::new();

        loop {
            let mut body: ChannelResponse = self.client.post("https://slack.com/api/conversations.list")
                .form(&params)
                .send()?
                .json()?;

            let cursor = body.response_metadata.next_cursor;
            channels.append(&mut body.channels);
            if cursor.is_empty() {
                break;
            }
            params.insert("cursor", cursor);
        }

        Ok(channels)
    }

    pub fn post_message(&self, channel_id: &str, message: &str) -> Result<(), Box<dyn Error>> {
        let mut params = HashMap::new();

        params.insert("token", self.token.as_str());
        params.insert("channel", channel_id);
        params.insert("text", message);

        let resp = self.client.post("https://slack.com/api/chat.postMessage")
            .form(&params)
            .send()?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(resp.status().as_str().into())
        }
    }
}
