#[macro_use]
extern crate rocket;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::{Datelike, NaiveTime};
use rand::seq::IndexedRandom;
use rocket::State;
use rocket::form::Form;
use rocket::response::status::{Accepted, BadRequest};
use rocket::serde::json::Json;
use serde::Serialize;
use slack::client::SlackClient;
use slack::config::Configuration;
use slack::events::{IT_IS_TODAY, SlackRequest, SlackState, time_report_message};
use slack::holidays::is_work_free;
use slack::last_day::{is_last_workday, next_time_report_day, now, today};
use tokio::time::sleep;

const REMINDER_CHANNEL: &str = "allmant";

#[rocket::main]
async fn main() -> ExitCode {
    // Load environment variables from .env
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    let has_arg = |arg: &str| args.iter().any(|a| a == arg);
    let run_api = has_arg("--operation=api");
    let check_last_workday = has_arg("--operation=check_last_workday");

    if !run_api && !check_last_workday {
        eprintln!("Please specify an --operation: api or check_last_workday");
        println!("Usage: joel-bot --operation=<operation>");
        println!("Operations:");
        println!("  api                   Start the Slack API server");
        println!(
            "  check_last_workday    Check if today is the last workday of the month and send a message to Slack if so"
        );
        return ExitCode::FAILURE;
    }

    let setup = || anyhow::Ok((Configuration::read()?, SlackClient::new()?));
    let (config, client) = match setup() {
        Ok((config, client)) => (Arc::new(config), Arc::new(client)),
        Err(error) => {
            eprintln!("couldn't start: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    if run_api {
        rocket::build()
            .manage(SlackState::new(client, config))
            .mount("/", routes![slack_request, time_report, gg])
            .launch()
            .await
            .expect("Server failed to start");
        return ExitCode::SUCCESS;
    }

    match last_workday_message(&config, &client).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("check_last_workday failed: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn last_workday_message(config: &Configuration, client: &SlackClient) -> anyhow::Result<()> {
    let today = today();
    if !is_last_workday(&today).await? {
        println!("Not last work day");
        return Ok(());
    }

    let message = config.get_message(&today.month().to_string());
    let channel_id = client
        .get_channel_id_by_name(REMINDER_CHANNEL)
        .await?
        .with_context(|| format!("no channel with name '{REMINDER_CHANNEL}' found"))?;
    client.post_message(&channel_id, &message).await?;
    println!("Posted time report reminder to #{REMINDER_CHANNEL}");
    Ok(())
}

#[post("/slack-request", format = "application/json", data = "<request>")]
fn slack_request(state: &State<SlackState>, request: Json<SlackRequest>) -> String {
    state.handle_request(request.into_inner())
}

// More information here: https://api.slack.com/interactivity/slash-commands
#[derive(FromForm)]
struct SlackSlashMessage {
    // token: String, <-- We should save and validate this
    // command: String, <-- can be used to check what command was used.
    // text: Option<String>,
    response_url: String,
}

#[derive(Serialize)]
struct SlashResponse {
    text: String,
}

const CALCULATIONS: [&str; 10] = [
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

#[post(
    "/time-report",
    format = "application/x-www-form-urlencoded",
    data = "<request>"
)]
fn time_report(
    request: Form<SlackSlashMessage>,
) -> Result<Accepted<String>, BadRequest<&'static str>> {
    let response_url = request.into_inner().response_url;
    // Only answer to Slack, so the endpoint can't be used to send requests elsewhere
    if !response_url.starts_with("https://hooks.slack.com/") {
        eprintln!("rejected /time-report with response_url {response_url}");
        return Err(BadRequest("invalid response_url"));
    }

    tokio::spawn(async move {
        let today = today();

        let messages = match next_time_report_day(&today).await {
            Ok(last_workday) if last_workday == today => {
                let mut rng = rand::rng();
                let mut messages = vec![time_report_message(&last_workday)];
                for calculation in CALCULATIONS.sample(&mut rng, 2) {
                    messages.push(format!("... {calculation}"));
                }
                messages.push(String::from(IT_IS_TODAY));
                messages
            }
            Ok(last_workday) => vec![format!(
                "Nu har jag gjort diverse uppslag och scrape:at nätet och det är inte förrän *{last_workday}* som du behöver tidrapportera!"
            )],
            Err(error) => {
                eprintln!("failed to get last work day: {error:#}");
                vec![String::from("Misslyckades stenhårt...")]
            }
        };

        for text in messages {
            sleep_and_send_time_report_response(&response_url, text).await;
        }
    });

    Ok(Accepted(String::from("Ska ta en titt i kalendern...")))
}

async fn sleep_and_send_time_report_response(url: &str, text: String) {
    // To "fool" the user that we are actually calculating something
    sleep(Duration::from_secs(2)).await;

    match slack::http::CLIENT
        .post(url)
        .json(&SlashResponse { text })
        .send()
        .await
    {
        Ok(response) if !response.status().is_success() => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eprintln!("couldn't send time report message, Slack answered {status}: {body}");
        }
        Ok(_) => {}
        Err(error) => eprintln!("couldn't send time report message: {error:#}"),
    }
}

#[post(
    "/gg",
    format = "application/x-www-form-urlencoded",
    data = "<_request>"
)]
fn gg(_request: Form<SlackSlashMessage>) -> Accepted<String> {
    let workday_start = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
    let workday_end = NaiveTime::from_hms_opt(17, 0, 0).unwrap();

    let now = now();
    let current_time = now.time();

    let message = if is_work_free(now.date_naive()) {
        String::from("Det är ju ledigt idag! Stäng datorn och gå ut i solen :sunny:")
    } else if current_time < workday_start {
        let time_until_start = generate_formatted_duration(&(workday_start - current_time));
        format!("Var lugn! Du behöver inte börja jobba förrän om {time_until_start}")
    } else if current_time < workday_end {
        let time_left = generate_formatted_duration(&(workday_end - current_time));
        format!(
            "Nu är det bara {time_left} innan du kan packa ihop för dagen, tänk vad kul du kan ha i {time_left} till! :smiley:"
        )
    } else {
        let end_time = workday_end.format("%H:%M");
        format!("Klockan är efter {end_time}, stay calm och sluta jobba!")
    };

    Accepted(message)
}

fn generate_formatted_duration(duration: &chrono::Duration) -> String {
    let unit = |count: i64, singular: &str, plural: &str| match count {
        0 => None,
        1 => Some(format!("1 {singular}")),
        count => Some(format!("{count} {plural}")),
    };
    let hours = duration.num_hours();
    let minutes = unit(duration.num_minutes() % 60, "minut", "minuter");

    // Seconds are only shown when less than an hour remains
    let parts = if hours > 0 {
        [unit(hours, "timme", "timmar"), minutes]
    } else {
        [
            minutes,
            unit(duration.num_seconds() % 60, "sekund", "sekunder"),
        ]
    };
    let present_parts: Vec<String> = parts.into_iter().flatten().collect();

    if present_parts.is_empty() {
        String::from("mindre än en sekund")
    } else {
        present_parts.join(" och ")
    }
}

#[cfg(test)]
mod test {
    use super::generate_formatted_duration;
    use chrono::Duration;

    #[test]
    fn formatted_durations() {
        let format = |seconds| generate_formatted_duration(&Duration::seconds(seconds));
        assert_eq!(format(0), "mindre än en sekund");
        assert_eq!(format(1), "1 sekund");
        assert_eq!(format(61), "1 minut och 1 sekund");
        assert_eq!(format(125), "2 minuter och 5 sekunder");
        assert_eq!(format(3600), "1 timme");
        assert_eq!(format(3600 + 60 + 5), "1 timme och 1 minut");
        assert_eq!(format(2 * 3600 + 30 * 60), "2 timmar och 30 minuter");
    }
}
