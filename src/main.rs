#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::time::Duration;

use chrono::NaiveTime;
use clokwerk::Scheduler;
use clokwerk::Interval::Weekday;
use rocket_contrib::json::Json;

use crate::slack::*;
use crate::config::*;

mod last_day;
mod slack;
mod config;

fn main() {
    let config = Configuration::read();

    // Run scheduler
    let client = SlackClient::new("");
    let mut scheduler = Scheduler::with_tz(chrono::Utc);
    scheduler.every(Weekday)
        .at_time(NaiveTime::from_hms(12, 0, 0))
        .run(move || {
            client.send_reminder_if_last_work_day()
                .unwrap_or_else(|error| println!("Got error: {}", error))
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
            let client = SlackClient::new("");
            client.post_message(&event.channel[..], ":joel: Hej allihopa, det är jag som är jo3ll-bot")
                .unwrap_or_else(|error| println!("{}", error))
        }
    }

    String::from("OK")
}
