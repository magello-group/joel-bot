#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::error::Error;

use reqwest::blocking::{Client, Response};
use rocket::{Config, Data, Request};
use rocket::config::Environment;
use rocket::data::{FromData, Outcome, Transform, Transformed};
use rocket::data::FromDataSimple;
use rocket_contrib::json::Json;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackRequest {
    #[serde(rename = "url_verification")]
    Challenge(ChallengeRequest),
    #[serde(rename = "event_callback")]
    Event(EventRequest),
}

#[derive(Deserialize)]
struct ChallengeRequest {
    token: String,
    challenge: String,
}

#[derive(Deserialize)]
struct EventRequest {
    token: String,
    event: Event,
}

#[derive(Deserialize, Debug)]
struct AppMentionEvent {
    user: String,
    text: String,
    channel: String,
}

#[derive(Deserialize)]
struct ChallengeEvent {
    user: String,
    text: String,
    channel: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Event {
    #[serde(rename = "app_mention")]
    AppMentionEvent(AppMentionEvent)
}

fn main() {
    rocket::custom(Config::build(Environment::Development)
        .address("0.0.0.0")
        .unwrap()
    ).mount("/", routes![slack_request]).launch();
}

#[post("/slack-request", format = "application/json", data = "<request>")]
fn slack_request(request: Json<SlackRequest>) -> String {
    match request.0 {
        SlackRequest::Challenge(request) => handle_challenge_request(request),
        SlackRequest::Event(request) => handle_event_request(request)
    }
}

fn handle_challenge_request(request: ChallengeRequest) -> String {
    request.challenge
}

fn handle_event_request(request: EventRequest) -> String {
    match request.event {
        Event::AppMentionEvent(event) => {
            let client = Client::new();
            post_message(&client, &event.channel[..], ":joel: Hej allihopa, det är jag som är jo3ll-bot");
        }
    }

    String::from("OK")
}

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

fn post_message(client: &Client, channel_id: &str, message: &str) -> Result<(), Box<dyn Error>> {
    let mut params = HashMap::new();

    params.insert("token", TOKEN);
    params.insert("channel", channel_id);
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