use std::error::Error;

use reqwest::blocking::{Client, Response};
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Channel {
    id: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct ResponseMetadata {
    next_cursor: String
}

#[derive(Deserialize, Debug)]
struct ChannelResponse {
    ok: bool,
    channels: Vec<Channel>,
    response_metadata: ResponseMetadata,
}

const TOKEN: &str = "";

fn get_channels(client: &Client) -> Result<Vec<Channel>, Box<dyn Error>> {
    let mut params = HashMap::new();
    params.insert("token", String::from(TOKEN));
    params.insert("types", String::from("private_channel"));
    let mut channels = Vec::new();

    loop {
        let mut body: ChannelResponse = client.post("https://slack.com/api/conversations.list")
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

fn post_message(client: &Client, channel: &Channel, message: &str) -> Result<(), Box<dyn Error>> {
    let mut params = HashMap::new();

    params.insert("token", TOKEN);
    params.insert("channel", channel.id.as_str());
    params.insert("text", message);

    let resp = client.post("https://slack.com/api/chat.postMessage")
        .form(&params)
        .send()?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(resp.status().as_str().into())
    }
}

fn main() {
    let client = Client::new();
    let channels = get_channels(&client)
        .expect("fail");

    for channel in &channels {
        println!("{:?}", channel);
        if channel.name == "joel-bot" {
            post_message(&client, channel, "Hej allihopa, det är jag som är jo3ll-bot");
        }
    }

    println!("{:?}", channels.len());
}
