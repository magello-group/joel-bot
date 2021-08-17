use std::collections::HashMap;
use std::error::Error;

use reqwest::blocking::Client;
use serde::Deserialize;

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

pub trait SlackClientTrait {
    fn get_channel_id_by_name(&self, channel_name: &str) -> Option<String>;
    fn get_channels(&self) -> Result<Vec<Channel>, Box<dyn Error>>;
    fn post_message(&self, channel_id: &str, message: &str) -> Result<(), Box<dyn Error>>;
}

impl SlackClient {
    pub fn new() -> Result<SlackClient, Box<dyn Error>> {
        let token = std::env::var("JOEL_BOT_SLACK_TOKEN")?;
        Ok(SlackClient {
            client: Client::new(),
            token: String::from(token),
        })
    }
}

impl SlackClientTrait for SlackClient {
    fn get_channel_id_by_name(&self, channel_name: &str) -> Option<String> {
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

    fn get_channels(&self) -> Result<Vec<Channel>, Box<dyn Error>> {
        let mut params = HashMap::new();
        params.insert("token", self.token.clone());
        params.insert("types", String::from("private_channel,public_channel"));
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

    fn post_message(&self, channel_id: &str, message: &str) -> Result<(), Box<dyn Error>> {
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
