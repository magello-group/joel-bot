#[macro_use]
extern crate rocket;
use crate::last_day::{get_last_workday, is_last_workday};

// Add dotenv support
use dotenv::dotenv;
use rand::SeedableRng;

use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc};
use chrono_tz::Europe::Stockholm;
use clokwerk::Interval::Weekday;
use clokwerk::{AsyncScheduler, Job};

use crate::config::*;
use rand::rngs::SmallRng;
use rand::Rng;
use reqwest::Client;
use rocket::form::Form;
use rocket::response::status::Accepted;
use rocket::serde::json::Json;
use rocket::State;
use slack::client::*;
use slack::events::{SlackRequest, SlackState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::sleep;

mod config;
mod last_day;

#[rocket::main]
async fn main() {
    // Load environment variables from .env
    dotenv().ok();

    let config = Arc::new(Configuration::read().expect("couldn't read configuration file"));
    let client = Arc::new(SlackClient::new().expect("couldn't initiate slack client"));

    // Run scheduler
    let mut scheduler = AsyncScheduler::with_tz(Utc);
    scheduler
        .every(Weekday)
        .at_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
        .run(move || last_workday_message(config.clone(), client.clone()));

    tokio::spawn(async move {
        loop {
            scheduler.run_pending().await;
            sleep(Duration::from_secs(60)).await;
        }
    });

    // Setup slack_events handler
    let slack_events = SlackState::new();

    // Start web server
    rocket::build()
        .manage(slack_events)
        .mount("/", routes![slack_request, time_report, gg])
        .launch()
        .await
        .expect("Server failed to start");
}

async fn last_workday_message(config: Arc<Configuration>, client: Arc<SlackClient>) {
    let today = Utc::now().date_naive();
    match is_last_workday(&today).await {
        Ok(true) => {
            let context = today.month().to_string();
            let message = config.get_message(&context);
            match client.get_channel_id_by_name("allmant").await {
                Some(channel_id) => {
                    if let Err(error) = client.post_message(&channel_id, &message).await {
                        println!("couldn't post message: {}", error)
                    }
                }
                None => println!("no channel with name 'allmant' found!"),
            }
        }
        Ok(false) => println!("Not last work day"),
        Err(_) => {
            // TODO Maybe handle error or nah?
        }
    };
}

#[post("/slack-request", format = "application/json", data = "<request>")]
async fn slack_request(state: &State<SlackState>, request: Json<SlackRequest>) -> String {
    let slack_request_data = request.into_inner();
    state.handle_request(slack_request_data).await
}

// More information here: https://api.slack.com/interactivity/slash-commands
#[derive(FromForm)]
struct SlackSlashMessage {
    // token: String, <-- We should save and validate this
    // command: String, <-- can be used to check what command was used.
    // text: Option<String>,
    response_url: String,
}

#[post(
    "/time-report",
    format = "application/x-www-form-urlencoded",
    data = "<request>"
)]
async fn time_report(request: Form<SlackSlashMessage>) -> Accepted<String> {
    let response_url = request.response_url.clone();

    let calculations = [
        "vänta",
        "beräknar",
        "processerar",
        "finurlar",
        "gnuggar halvledarna",
        "tömmer kvicksilver-depå",
        "springer i cirklar",
        "kryssar och jämför",
        "skruvar och muttrar",
        "går på djupet",
    ];

    tokio::spawn(async move {
        let today = Utc::now().naive_utc().date();
        let http_client = Client::new();
        let mut map = HashMap::new();
        let mut rng = SmallRng::from_os_rng();

        match get_last_workday(&today).await {
            Ok(last_workday) => {
                if last_workday == today {
                    map.insert("text", format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!", last_workday));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map).await;

                    for _ in 0..2 {
                        let pos = rng.random_range(0..calculations.len());

                        map.insert("text", format!("... {}", calculations[pos]));

                        sleep_and_send_time_report_response(&http_client, &response_url, &map).await;
                    }

                    map.insert("text", String::from("... det är ju idag!"));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map).await;
                } else {
                    map.insert("text", format!("Nu har jag gjort diverse uppslag och scrape:at nätet och det är inte förrän *{}* som du behöver tidrapportera!", last_workday));

                    sleep_and_send_time_report_response(&http_client, &response_url, &map).await
                }
            }
            Err(error) => {
                println!("failed to get last work day: {}", error);

                map.insert("text", String::from("Misslyckades stenhårt..."));
                sleep_and_send_time_report_response(&http_client, &response_url, &map).await;
            }
        };
    });

    Accepted("Ska ta en titt i kalendern...".to_string())
}

async fn sleep_and_send_time_report_response(
    http_client: &Client,
    url: &String,
    map: &HashMap<&str, String>,
) {
    // To "fool" the user that we are actually calculating something
    sleep(Duration::from_secs(2)).await;

    let resp = http_client.post(url.as_str()).json(map).send();

    match resp.await {
        Ok(r) => {
            if !r.status().is_success() {
                println!("failed to send message, {}", r.status().as_str());
                let result = r.text().await;
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

#[post(
    "/gg",
    format = "application/x-www-form-urlencoded",
    data = "<request>"
)]
async fn gg(request: Form<SlackSlashMessage>) -> Accepted<String> {
    let upper = NaiveTime::from_hms_opt(17, 0, 0).unwrap();
    let lower = NaiveTime::from_hms_opt(8, 0, 0).unwrap();

    let date = Utc::now();
    let time = date.with_timezone(&Stockholm).time();

    let message = if time < upper && time >= lower {
        let delta = upper - time;
        let string = generate_formatted_duration(&delta);
        format!("Nu är det bara {} innan du kan packa ihop för dagen, tänk vad kul du kan ha i {} till! :smiley:", string, string)
    } else if time < lower {
        let delta = lower - time;
        let string = generate_formatted_duration(&delta);
        format!("Var lugn! Du behöver inte börja jobba förrän om {}", string)
    } else {
        format!("Klockan är efter {}, stay calm och sluta jobba!", upper.format("%H:%M"))
    };

    Accepted(message)
}

fn generate_formatted_duration(duration: &chrono::Duration) -> String {
    let mut formatted = String::new();
    let seconds = duration.num_seconds() % 60;
    let minutes = duration.num_minutes() % 60;
    let hours = duration.num_hours();

    let append = |s1: &str, s2: &str| -> String {
        return if s1.len() > 0 {
            format!("{} och {}", s1, s2)
        } else {
            String::from(s2)
        }
    };

    match minutes {
        1 => formatted = append(&formatted, format!("{} minut", minutes).as_str()),
        minutes if minutes > 1 => formatted = append(&formatted, format!("{} minuter", minutes).as_str()),
        _ => {}
    };

    match hours {
        1 => formatted = append(&format!("{} timme", hours), formatted.as_str()),
        hours if hours > 1 => formatted = append(&format!("{} timmar", hours), formatted.as_str()),
        _ => {
            // Add seconds only when hours are < 1
            match seconds {
                1 => formatted = append(&formatted, format!("{} sekund", seconds).as_str()),
                seconds if seconds > 1 => formatted = append(&formatted, format!("{} sekunder", seconds).as_str()),
                _ => {}
            };
        }
    };

    formatted
}
