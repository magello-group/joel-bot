use std::collections::HashMap;

use reqwest::{Client};
use serde::Deserialize;
use anyhow::Result;

#[derive(Deserialize, Debug)]
pub struct Channel {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct ResponseMetadata {
    next_cursor: String,
}

#[derive(Deserialize, Debug)]
pub struct ChannelResponse {
    #[allow(dead_code)]
    ok: bool,
    channels: Vec<Channel>,
    response_metadata: ResponseMetadata,
}

pub struct SlackClient {
    client: Client,
    token: String,
}

#[async_trait::async_trait]
pub trait SlackClientTrait {
    async fn get_channel_id_by_name(&self, channel_name: &str) -> Option<String>;
    async fn get_channels(&self) -> Result<Vec<Channel>>;
    async fn post_message(&self, channel_id: &str, message: &str) -> Result<()>;
}

impl SlackClient {
    pub fn new() -> Result<SlackClient> {
        let token = std::env::var("JOEL_BOT_SLACK_TOKEN")?;
        Ok(SlackClient {
            client: Client::new(),
            token: String::from(token),
        })
    }
}

#[async_trait::async_trait]
impl SlackClientTrait for SlackClient {
    async fn get_channel_id_by_name(&self, channel_name: &str) -> Option<String> {
        match self.get_channels().await {
            Ok(channels) => channels
                .into_iter()
                .find(|channel| channel.name == channel_name)
                .map(|channel| channel.id),
            Err(error) => {
                println!("{}", error);
                None
            }
        }
    }

    async fn get_channels(&self) -> Result<Vec<Channel>> {
        let mut params = HashMap::new();
        params.insert("token", self.token.clone());
        params.insert("types", String::from("private_channel,public_channel"));
        let mut channels = Vec::new();

        loop {
            let response: ChannelResponse = self
                .client
                .post("https://slack.com/api/conversations.list")
                .form(&params)
                .send()
                .await?
                .json()
                .await?;

            channels.extend(response.channels);
            if response.response_metadata.next_cursor.is_empty() {
                break;
            }
            params.insert("cursor", response.response_metadata.next_cursor);
        }

        Ok(channels)
    }

    async fn post_message(&self, channel_id: &str, message: &str) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("token", self.token.clone());
        params.insert("channel", channel_id.to_string());
        params.insert("text", message.to_string());

        // let result = self
        self.client
            .post("https://slack.com/api/chat.postMessage")
            .form(&params)
            .send()
            .await?
            .error_for_status()?;

        Ok(())

    }
}

