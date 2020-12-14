#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc};
use clokwerk::Scheduler;
use clokwerk::Interval::Weekday;
use rocket_contrib::json::Json;

use crate::config::*;
use crate::last_day::is_last_workday;
use crate::slack::*;

mod last_day;
mod slack;
mod config;

fn main() {
    let config = Configuration::read()
        .expect("couldn't read configuration file");
    let client = SlackClient::new()
        .expect("couldn't initiate slack client");

    //
    // Features
    // - Sensible messages
    // - @joel-bot - Give presentation, explain features and his existential value

    println!("{}", config.get_introduction());

    // Run scheduler
    let mut scheduler = Scheduler::with_tz(chrono::Utc);
    scheduler.every(Weekday)
        .at_time(NaiveTime::from_hms(12, 0, 0))
        .run(move || {
            let now = Utc::now();
            match is_last_workday(&now) {
                Ok(true) => {
                    let context = now.date().month().to_string();
                    let message = config.get_message(&context);
                    match client.get_channel_id_by_name("allmant") {
                        Some(channel_id) => {
                            if let Err(error) = client.post_message(&channel_id, &message) {
                                println!("couldn't post message: {}", error)
                            }
                        }
                        None => {}
                    }
                }
                Ok(false) => {
                    println!("not last work day")
                }
                Err(error) => {
                    println!("{}", error);
                }
            }
        });
    let _schedule_handle = scheduler.watch_thread(Duration::from_millis(100));

    // Start web server
    rocket::ignite().mount("/", routes![slack_request]).launch();
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
            let config = Configuration::read()
                .expect("couldn't read configuration when mentioned");
            let client = SlackClient::new()
                .expect("couldn't initiate slack client");
            client.post_message(&event.channel[..], &config.get_introduction())
                .unwrap_or_else(|error| println!("{}", error))
        }
    }

    String::from("OK")
}
