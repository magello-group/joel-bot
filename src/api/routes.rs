use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use rand::{Rng, thread_rng};
use reqwest::blocking::Client;
use rocket::FromForm;
use rocket::http::Status;
use rocket::post;
use rocket::request::LenientForm;
use rocket::State;
use rocket_contrib::json::Json;
use serde::Serialize;

use crate::api::{SlackBlockResponse, SlackErrorResponse, SLApi, SLApiKeys, SLStationInfo, SLStationInfoResponse};
use crate::events::{SlackEvents, SlackRequest};
use crate::last_day::get_last_workday;

#[post("/take-me-home", format = "application/x-www-form-urlencoded", data = "<request>")]
pub fn handle_trip_command(state: State<SLApiKeys>, request: LenientForm<SlackSlashMessage>) -> String {
    let stations: Vec<&str> = request.text.split(' ').collect();
    if stations.len() != 2 {
        println!("Got too few stations");
        return "Jag behöver två argument fattaru la? Ex: `/gg t-centr fruän`".to_string();
    };

    let from_name = stations[0].to_string();
    let to_name = stations[1].to_string();

    let trip_token = state.get_trip_token.clone();
    let stations_token = state.get_stations_token.clone();
    thread::spawn(move || {
        let http_client = Client::new();

        let from_result = SLApi::read_station(&http_client, &stations_token, &from_name, 1);
        let to_result = SLApi::read_station(&http_client, &stations_token, &to_name, 1);

        let from_station = match get_first_station(from_result) {
            Some(f) => f,
            None => {
                println!("Could not find a station with name: {}", from_name);
                send_json_response(&http_client, &request.response_url, &SlackErrorResponse::new(format!("Hittade ingen station med namnet och {}", &from_name)));
                return;
            }
        };

        let to_station = match get_first_station(to_result) {
            Some(t) => t,
            None => {
                println!("Could not find a station with name: {}", to_name);
                send_json_response(&http_client, &request.response_url, &SlackErrorResponse::new(format!("Hittade ingen station med namnet {}", &to_name)));
                return;
            }
        };

        let result = SLApi::list_trips(&http_client, &trip_token, &from_station.site_id, &to_station.site_id);

        let result = match result {
            Ok(trip_response) => {
                SlackBlockResponse::create_trip_response(&from_name, &to_name, &trip_response)
            }
            Err(error) => {
                println!("Error: {}", error);
                send_json_response(&http_client, &request.response_url, &SlackErrorResponse::new(format!("Hittade ingen resa mellan {} och {}", &from_name, &to_name)));
                return;
            }
        };

        send_json_response(&http_client, &request.response_url, &result)
    });

    String::from("Låt mig se efter om det finns en resa hos SL åt dig!")
}

fn get_first_station(result: Result<SLStationInfoResponse, Box<dyn Error>>) -> Option<SLStationInfo> {
    match result {
        Ok(to) => {
            let data = to.response_data.unwrap();
            match data.get(0) {
                Some(t) => Some(t.clone()),
                None => {
                    // TODO: Send error
                    None
                }
            }
        }
        Err(_error) => {
            // TODO: Send error
            None
        }
    }
}

#[post("/slack-request", format = "application/json", data = "<request>")]
pub fn slack_request(state: State<SlackEvents>, request: Json<SlackRequest>) -> String {
    state.handle_request(request.0)
}

// More information here: https://api.slack.com/interactivity/slash-commands
#[derive(FromForm)]
pub struct SlackSlashMessage {
    // token: String, <-- We should save and validate this
    // command: String, <-- can be used to check what command was used.
    text: String,
    // <-- Seems to exists even if it is empty
    response_url: String,
}

#[post("/time-report", format = "application/x-www-form-urlencoded", data = "<request>")]
pub fn time_report(request: LenientForm<SlackSlashMessage>) -> String {
    let response_url = request.response_url.clone();

    let calculations = vec!["vänta", "beräknar", "processerar", "finurlar", "gnuggar halvledarna", "tömmer kvicksilver-depå"];

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
                        let pos = rng.gen_range(0, calculations.len() - 1);

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

fn sleep_and_send_time_report_response(http_client: &Client, url: &String, map: &HashMap<&str, String>) {
    // To "fool" the user that we are actually calculating something
    thread::sleep(Duration::from_secs(2));

    send_response(http_client, url, map)
}

fn send_response(http_client: &Client, url: &String, map: &HashMap<&str, String>) {
    let resp = http_client.post(url.as_str())
        .json(map)
        .send();

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

fn send_json_response(http_client: &Client, url: &String, data: &impl Serialize) {
    let resp = http_client.post(url.as_str())
        .json(data)
        .send();

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
