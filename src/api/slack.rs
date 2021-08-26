use serde::{Serialize};
use crate::api::{SLWalk, SLVehicle, SLCategory, SLStation, SLLeg, SLTrip, SLTripResponse};
use chrono::{NaiveDateTime, DateTime, Utc};

use chrono_tz::Europe::Stockholm;
use chrono_tz::{Tz, OffsetComponents};

const MAX_TRIPS: usize = 3;

#[derive(Serialize, Debug)]
pub struct SlackBlockResponse {
    pub(crate) blocks: Vec<SlackBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) response_type: Option<String>,
}

impl SlackBlockResponse {
    pub fn create_trip_response(from: &str, to: &str, trip_response: &SLTripResponse) -> SlackBlockResponse {
        let mut blocks = Vec::new();
        blocks.push(SlackBlock {
            text: Some(SlackText {
                text_type: "mrkdwn".to_string(),
                text: format!("*Sökning:* {} _till_ {}", from, to),
            }),
            block_type: "section".to_string(),
            ..Default::default()
        });
        blocks.push(SlackBlock {
            elements: Some(vec!(
                SlackPlaceholder {
                    placeholder_type: "mrkdwn".to_string(),
                    text: ":warning: *Tänk på att vissa byten kan innehålla förseningar*".to_string(),
                    emoji: None,
                }
            )),
            block_type: "context".to_string(),
            ..Default::default()
        });
        let mut trips = trip_response.trip.iter()
            .take(MAX_TRIPS)
            .map(|trip| {
                let mut blocks = Vec::new();
                blocks.push(SlackBlock {
                    block_type: "divider".to_string(),
                    ..Default::default()
                });
                blocks.append(&mut SlackBlock::create_trip_block(trip));
                blocks.push(SlackBlock::total_travel_time(trip));
                return blocks;
            })
            .flatten()
            .collect::<Vec<SlackBlock>>();

        blocks.append(&mut trips);

        SlackBlockResponse {
            blocks,
            response_type: Some(String::from("ephemeral")),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct SlackBlock {
    #[serde(rename(serialize = "type"))] // section
    pub(crate) block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<SlackText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) accessory: Option<SlackAccessory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) elements: Option<Vec<SlackPlaceholder>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fields: Option<Vec<SlackText>>,
}

impl Default for SlackBlock {
    fn default() -> Self {
        SlackBlock {
            block_type: "section".to_string(),
            text: None,
            accessory: None,
            elements: None,
            fields: None,
        }
    }
}

impl SlackBlock {
    pub fn total_travel_time(trip: &SLTrip) -> SlackBlock {
        // We know that the trip will have at least one leg element, so we can unwrap both
        let first = trip.leg_list.legs.first()
            .unwrap();
        let last = trip.leg_list.legs.last()
            .unwrap();

        let origin = match first {
            SLLeg::JNY(vehicle) => parse_date_from_string(&vehicle.origin),
            SLLeg::WALK(walk) => parse_date_from_string(&walk.origin),
        };

        let dest = match last {
            SLLeg::JNY(vehicle) => parse_date_from_string(&vehicle.destination),
            SLLeg::WALK(walk) => parse_date_from_string(&walk.destination),
        };

        let start = origin.format("%R"); // Only time
        let stop = dest.format("%R"); // Only time

        let dest_backup = dest.clone();

        let now: DateTime<Tz> = Utc::now().with_timezone(&Stockholm);

        let duration = dest.signed_duration_since(origin);
        let duration_to_final_destination = dest_backup.signed_duration_since(now);

        SlackBlock {
            block_type: "context".to_string(),
            elements: Some(vec!(
                SlackPlaceholder {
                    text: format!(
                        "*Start*: {} - *Framme*: {} - *Total restid*: {}\nTar du denna resa är du framme om ungefär {}",
                        start,
                        stop,
                        generate_formatted_duration(&duration),
                        generate_formatted_duration(&duration_to_final_destination)
                    ),
                    placeholder_type: "mrkdwn".to_string(),
                    emoji: None,
                }
            )),
            ..Default::default()
        }
    }

    pub fn create_trip_block(trip: &SLTrip) -> Vec<SlackBlock> {
        trip.leg_list.legs.iter()
            .filter(|leg| {
                match leg {
                    SLLeg::WALK(walk) => walk.hide.contains(&false), // Skip all hidden walks
                    SLLeg::JNY(_) => true
                }
            })
            .enumerate()
            .map(|(i, leg)| {
                match leg {
                    SLLeg::WALK(walk) => {
                        if walk.hide.contains(&true) {}
                        SlackBlock::from_walk(i, walk)
                    }
                    SLLeg::JNY(vehicle) => {
                        SlackBlock::from_vehicle(i, vehicle)
                    }
                }
            })
            .flatten()
            .collect::<Vec<SlackBlock>>()
    }

    pub fn from_walk(i: usize, walk: &SLWalk) -> Vec<SlackBlock> {
        vec![
            SlackBlock {
                block_type: "section".to_string(),
                text: Some(SlackText {
                    text_type: "mrkdwn".to_string(),
                    text: format!("{} Gå :walking:", get_slack_emoji_from_number(i + 1)),
                }),
                ..Default::default()
            },
            SlackBlock {
                block_type: "section".to_string(),
                fields: Some(vec![
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Från*\n{}", walk.origin.name),
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Till*\n{}", walk.destination.name),
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Tid att gå:*\n{}", walk.duration), // Format to minutes and seconds
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Avstånd:*\n{} meter", walk.dist),
                    },
                ]),
                ..Default::default()
            },
        ]
    }

    pub fn from_vehicle(i: usize, vehicle: &SLVehicle) -> Vec<SlackBlock> {
        vec![
            SlackBlock {
                block_type: "section".to_string(),
                text: Some(SlackText {
                    text_type: "mrkdwn".to_string(),
                    text: format!("{} *{}* mot *{}* {}",
                                  get_slack_emoji_from_number(i + 1),
                                  some_kind_of_uppercase_first_letter(vehicle.name.as_str()),
                                  vehicle.direction,
                                  match vehicle.category {
                                      SLCategory::MET => String::from(":metro:"),
                                      SLCategory::BUS => String::from(":bus:"),
                                      SLCategory::TRN => String::from(":bullettrain_front:"),
                                      SLCategory::TRM => String::from(":tram:"),
                                      SLCategory::UUU => String::from(":thonking:")
                                  }
                    ),
                }),
                ..Default::default()
            },
            SlackBlock {
                block_type: "section".to_string(),
                fields: Some(vec![
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Från*\n{}", vehicle.origin.name),
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Till*\n{}", vehicle.destination.name),
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Avgår (preliminärt):*\n{}", generate_timestamps_from_vehicle(&vehicle.origin)), // Format to minutes and seconds
                    },
                    SlackText {
                        text_type: "mrkdwn".to_string(),
                        text: format!("*Framme (preliminärt):*\n{}", generate_timestamps_from_vehicle(&vehicle.destination)),
                    },
                ]),
                ..Default::default()
            },
        ]
    }
}

fn generate_timestamps_from_vehicle(vehicle_point: &SLStation) -> String {
    let time = vehicle_point.get_time();
    let date = vehicle_point.get_date();

    // TODO: Handle late metros
    match NaiveDateTime::parse_from_str(
        format!("{} {}", date, time).as_str(),
        "%Y-%m-%d %H:%M:%S",
    ) {
        Ok(time) => {
            format!("<!date^{}^{{date_short_pretty}} {}|{}>", time.timestamp(), time.format("%R"), time.format("%F %R"))
        },
        Err(_err) => format!("{} {}", vehicle_point.date, vehicle_point.time)
    }
}

fn get_slack_emoji_from_number(i: usize) -> String {
    match i {
        1 => String::from(":one:"),
        2 => String::from(":two:"),
        3 => String::from(":three:"),
        4 => String::from(":four:"),
        5 => String::from(":five:"),
        6 => String::from(":six:"),
        7 => String::from(":seven:"),
        8 => String::from(":eight:"),
        9 => String::from(":nine:"),
        _ => String::from(":1234:") // Unknown value
    }
}

fn parse_date_from_string(vehicle_point: &SLStation) -> DateTime<Tz> {
    let now = Utc::now().with_timezone(&Stockholm);
    let offset_hours = now.offset().base_utc_offset().num_hours() + now.offset().dst_offset().num_hours();

    match DateTime::parse_from_str(
        format!("{} {} +0{}00", vehicle_point.get_date(), vehicle_point.get_time(), offset_hours).as_str(),
        "%Y-%m-%d %H:%M:%S %z",
    ) {
        Ok(time) => time.with_timezone(&Stockholm),
        Err(_err) => Utc::now().with_timezone(&Stockholm)
    }
}

fn generate_formatted_duration(duration: &chrono::Duration) -> String {
    let minutes = duration.num_minutes() % 60;
    let hours = duration.num_hours();
    let hours = match hours {
        1 => format!("{} timme", hours),
        hours if hours > 1 => format!("{} timmar", hours),
        _ => String::from("")
    };

    let minutes = match minutes {
        1 => format!("{} minut", minutes),
        minutes if minutes > 1 => format!("{} minuter", minutes),
        _ => String::from("N/A")
    };

    match hours.len() {
        0 => minutes,
        _ => format!("{} och {}", hours, minutes)
    }
}

// Stole it from Stack overflow: https://stackoverflow.com/questions/38406793/why-is-capitalizing-the-first-letter-of-a-string-so-convoluted-in-rust
fn some_kind_of_uppercase_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// text cannot be empty, the API will return error status for an empty String.
#[derive(Serialize, Debug)]
pub struct SlackText {
    #[serde(rename(serialize = "type"))]
    pub(crate) text_type: String,
    // mrkdwn or plain_text
    pub(crate) text: String,
}

#[derive(Serialize, Debug)]
pub struct SlackAccessory {
    #[serde(rename(serialize = "type"))] // static_select
    pub(crate) accessory_type: String,
    pub(crate) placeholder: Option<SlackPlaceholder>,
    pub(crate) options: Vec<SlackOption>,
    pub(crate) action_id: String,
}

impl Default for SlackAccessory {
    fn default() -> Self {
        SlackAccessory {
            accessory_type: "static_select".to_string(),
            placeholder: None,
            options: vec![],
            action_id: "".to_string(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct SlackOption {
    pub(crate) text: SlackPlaceholder,
    pub(crate) value: String,
}

#[derive(Serialize, Debug)]
pub struct SlackPlaceholder {
    #[serde(rename(serialize = "type"))] // plain_text
    pub(crate) placeholder_type: String,
    pub(crate) text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) emoji: Option<bool>,
}

impl Default for SlackPlaceholder {
    fn default() -> Self {
        SlackPlaceholder {
            placeholder_type: "plain_text".to_string(),
            text: "".to_string(),
            emoji: None,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct SlackErrorResponse {
    text: String,
    response_type: String,
}

impl SlackErrorResponse {
    pub fn new(message: String) -> SlackErrorResponse {
        SlackErrorResponse {
            text: message,
            response_type: String::from("ephemeral"),
        }
    }
}
