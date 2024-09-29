#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc};
use clokwerk::Interval::Weekday;
use clokwerk::{Job, Scheduler};

use crate::config::*;
use crate::last_day::{get_last_workday, is_last_workday};
use rand::{thread_rng, Rng};
use reqwest::blocking::Client;
use rocket::form::Form;
use rocket::State;
use rocket::serde::{json::Json, Deserialize};
use slack::client::*;
use slack::events;
use slack::events::{SlackEvents, SlackRequest};
use std::collections::HashMap;
use std::thread;

mod config;
mod last_day;

#[rocket::main]
async fn main() {
    let config = Configuration::read().expect("couldn't read configuration file");
    let client = SlackClient::new().expect("couldn't initiate slack client");

    // Run scheduler
    let mut scheduler = Scheduler::with_tz(Utc);
    scheduler
        .every(Weekday)
        .at_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
        .run(move || {
            let now = Utc::now();
            match is_last_workday(&now) {
                Ok(true) => {
                    let context = now.date_naive().month().to_string();
                    let message = config.get_message(&context);
                    match client.get_channel_id_by_name("allmant") {
                        Some(channel_id) => {
                            if let Err(error) = client.post_message(&channel_id, &message) {
                                println!("couldn't post message: {}", error)
                            }
                        }
                        None => println!("no channel with name 'allmant' found!"),
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
    let _schedule_handle = scheduler.watch_thread(Duration::from_secs(60));

    // Setup slack_events handler
    let mut slack_events = events::SlackEvents::new();
    slack_events.set_mention_event_handler(handle_mention_event);

    // Start web server
    rocket::build()
        .manage(slack_events)
        .mount("/", routes![slack_request, time_report])
        .launch()
        .await
        .expect("Server failed to start");
}

#[post("/slack-request", format = "application/json", data = "<request>")]
async fn slack_request(state: &State<SlackEvents>, request: Json<SlackRequest>) -> String {
    let slack_request_data = request.into_inner();
    state.handle_request(slack_request_data)
}

// More information here: https://api.slack.com/interactivity/slash-commands
#[derive(FromForm)]
struct SlackSlashMessage {
    // token: String, <-- We should save and validate this
    // command: String, <-- can be used to check what command was used.
    text: Option<String>,
    response_url: String,
}

#[post(
    "/time-report",
    format = "application/x-www-form-urlencoded",
    data = "<request>"
)]
fn time_report(request: Form<SlackSlashMessage>) -> String {
    let response_url = request.response_url.clone();

    let calculations = vec![
        "vänta",
        "beräknar",
        "processerar",
        "finurlar",
        "gnuggar halvledarna",
        "tömmer kvicksilver-depå",
    ];

    thread::spawn(move || {
        let now = Utc::now();
        let http_client = Client::new();
        let mut map = HashMap::new();

        match get_last_workday(&now) {
            Ok(last_workday) => {
                if last_workday == now.naive_utc().date() {
                    map.insert("text", format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!", last_workday));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map);

                    let mut rng = thread_rng();
                    for _ in 0..2 {
                        let pos = rng.gen_range(0..calculations.len());

                        map.insert("text", format!("... {}", calculations[pos]));

                        sleep_and_send_time_report_response(&http_client, &response_url, &map);
                    }

                    map.insert("text", String::from("... det är ju idag!"));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map);
                } else {
                    map.insert("text", format!("Nu har jag gjort diverse uppslag och scrape:at nätet och det är inte förrän *{}* som du behöver tidrapportera!", last_workday));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map)
                }
            }
            Err(error) => {
                println!("failed to get last work day: {}", error);

                map.insert("text", String::from("Misslyckades stenhårt..."));
                sleep_and_send_time_report_response(&http_client, &response_url, &map)
            }
        };
    });

    format!("Ska ta en titt i kalendern...")
}

fn sleep_and_send_time_report_response(
    http_client: &Client,
    url: &String,
    map: &HashMap<&str, String>,
) {
    // To "fool" the user that we are actually calculating something
    thread::sleep(Duration::from_secs(2));

    let resp = http_client.post(url.as_str()).json(map).send();

    match resp {
        Ok(r) => {
            if !r.status().is_success() {
                println!("failed to send message, {}", r.status().as_str());
                let result = r.text();
                if result.is_ok() {
                    println!("{}", result.unwrap());
                }
            }
        }
        Err(err) => {
            println!("got exception while sending message: {}", err)
        }
    }
}

fn handle_mention_event(client: &impl SlackClientTrait, event: events::AppMentionEvent) -> String {
    let config = Configuration::read().expect("couldn't read configuration when mentioned");

    let mut splits: Vec<&str> = event.text.split(" ").collect();
    splits.drain(0..1);

    let message: String = if splits.len() > 0 {
        match splits[0] {
            "tid" => {
                let now = Utc::now();
                match get_last_workday(&now) {
                    Ok(last_workday) => {
                        if last_workday == now.naive_utc().date() {
                            format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!\n\n... vänta\n... beräknar\n... det är ju idag!", last_workday)
                        } else {
                            format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!", last_workday)
                        }
                    }
                    Err(error) => {
                        println!("{}", error);
                        String::from("Herregud någonting gick skitfel! Jag kanske behöver uppdatera min firmware :joel:. Kan någon snälla kolla loggen i Azure?")
                    }
                }
            }
            "pricing" => {
                String::from("För den nätta kostnaden av 114,805 kr per månad eller 15,8 öre per timme kan du hosta din egen joel-bot! :joel:")
            }
            "skribenter" => {
                config.get_authors()
            }
            _command => {
                format!("Är du skön eller <@{}>? Tror du att _jag_ vet något om *{}*? :joel:", event.user, splits.join(" "))
            }
        }
    } else {
        config.get_introduction()
    };
    client
        .post_message(&event.channel, &message)
        .unwrap_or_else(|error| println!("{}", error));

    String::new()
}
