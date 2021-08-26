use std::fmt::{Display, Formatter};

use serde::{Serialize, Deserialize};
use reqwest::blocking::Client;
use std::error::Error;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct SLTripResponse {
    #[serde(rename(deserialize = "Trip"))]
    pub(crate) trip: Vec<SLTrip>,
}

#[derive(Deserialize, Debug)]
pub struct SLTrip {
    #[serde(rename(deserialize = "LegList"))]
    pub(crate) leg_list: SLLegList,
}

#[derive(Deserialize, Debug)]
pub struct SLLegList {
    #[serde(rename(deserialize = "Leg"))]
    pub(crate) legs: Vec<SLLeg>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum SLLeg {
    WALK(SLWalk),
    JNY(SLVehicle),
}

#[derive(Deserialize, Debug)]
pub struct SLVehicle {
    #[serde(rename(deserialize = "Origin"))]
    pub(crate) origin: SLStation,
    #[serde(rename(deserialize = "Destination"))]
    pub(crate) destination: SLStation,
    pub(crate) name: String,
    pub(crate) direction: String,
    pub(crate) category: SLCategory,
}

/// SLWalk doesn't contain category
#[derive(Deserialize, Debug)]
pub struct SLWalk {
    #[serde(rename(deserialize = "Origin"))]
    pub(crate) origin: SLStation,
    #[serde(rename(deserialize = "Destination"))]
    pub(crate) destination: SLStation,
    pub(crate) name: String,
    pub(crate) duration: String,
    pub(crate) dist: i32,
    pub(crate) hide: Option<bool>, // Can be missing... What?
}

#[derive(Deserialize, Debug)]
pub enum SLCategory {
    TRN,
    TRM,
    BUS,
    MET,
    UUU,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SLStation {
    pub(crate) name: String,
    pub(crate) time: String,
    // TODO: fix to real time
    pub(crate) date: String,
    #[serde(rename(deserialize = "rtTime"))]
    pub(crate) rt_time: Option<String>,
    #[serde(rename(deserialize = "rtDate"))]
    pub(crate) rt_date: Option<String>,
}

impl SLStation {
    pub fn get_time(&self) -> String {
        match &self.rt_time {
            Some(real_time) => real_time.clone(),
            None => self.time.clone()
        }
    }

    pub fn get_date(&self) -> String {
        match &self.rt_date {
            Some(real_date) => real_date.clone(),
            None => self.date.clone()
        }
    }
}

// Station info, name and id.
#[derive(Deserialize, Debug)]
pub struct SLStationInfoResponse {
    #[serde(rename(deserialize = "StatusCode"))]
    pub(crate) status_code: i16,
    #[serde(rename(deserialize = "Message"))]
    pub(crate) message: Option<String>,
    #[serde(rename(deserialize = "ResponseData"))]
    pub(crate) response_data: Option<Vec<SLStationInfo>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SLStationInfo {
    #[serde(rename(deserialize = "Name"))]
    pub(crate) name: String,
    #[serde(rename(deserialize = "SiteId"))]
    pub(crate) site_id: String,
}

#[derive(Debug, Clone)]
pub struct SLError {
    pub(crate) message: String,
}

impl Display for SLError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.message)
    }
}

impl std::error::Error for SLError {}

impl SLError {
    pub fn new(message: String) -> SLError {
        SLError {
            message
        }
    }
}

pub struct SLApiKeys {
    pub get_trip_token: String,
    pub get_stations_token: String,
}

impl SLApiKeys {
    pub fn new() -> Result<SLApiKeys, Box<dyn Error>> {
        let get_trip_token = std::env::var("SL_TRIP_API_TOKEN")?;
        let get_stations_token = std::env::var("SL_STATION_LIST_API_TOKEN")?;

        Ok(SLApiKeys {
            get_trip_token,
            get_stations_token
        })
    }
}

pub struct SLApi {}

impl SLApi {
    pub fn list_trips(http_client: &Client, trip_token: &String, from: &str, to: &str) -> Result<SLTripResponse, Box<dyn Error>> {
        let mut query = HashMap::new();
        query.insert("key", trip_token.as_str());
        query.insert("originId", from);
        query.insert("destId", to);

        let sl_resp: SLTripResponse = http_client.get("https://api.sl.se/api2/TravelplannerV3_1/trip.json")
            .query(&query)
            .send()?
            .json()?;

        Ok(sl_resp)
    }

    /// If the Result is Ok, then the response_data contains Some.
    pub fn read_station(http_client: &Client, station_token: &String, name: &str, max_result: i32) -> Result<SLStationInfoResponse, Box<dyn Error>> {
        let mut query = HashMap::new();
        query.insert("key", station_token.as_str()); // <-- API Key
        query.insert("searchstring", name);
        query.insert("stationsonly", "false");

        let max_result_str = max_result.to_string();
        query.insert("maxresults", max_result_str.as_str());

        let sl_resp: SLStationInfoResponse = http_client.get("https://api.sl.se/api2/typeahead.json")
            .query(&query)
            .send()?
            .json()?;

        match sl_resp.status_code {
            0 => {
                if sl_resp.response_data.is_some() {
                    return Ok(sl_resp);
                }
                Err(SLError::new("got empty response data".to_string()).into())
            }
            _ => Err(SLError::new(sl_resp.message.unwrap()).into())
        }
    }
}