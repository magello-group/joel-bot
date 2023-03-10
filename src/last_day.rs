use std::error::Error;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct SholidayFaboulResponse {
    // TODO: Add if needed
    // #[serde(alias = "startdatum")]
    // start_date: String,
    // #[serde(alias = "slutdatum")]
    // end_date: String,
    #[serde(alias = "dagar")]
    days: Vec<SholidayFaboulDay>,
}

#[derive(Deserialize)]
struct SholidayFaboulDay {
    #[serde(alias = "datum")]
    date: String,
    #[serde(alias = "arbetsfri dag")]
    work_free_day: String,
}

pub fn is_last_workday(date: &DateTime<Utc>) -> Result<bool, Box<dyn Error>> {
    Ok(get_last_workday(date)? == date.naive_utc().date())
}

pub fn get_last_workday(date: &DateTime<Utc>) -> Result<NaiveDate, Box<dyn Error>> {
    let client = Client::new();
    let url = format!(
        "https://sholiday.faboul.se/dagar/v2.1/{}/{}",
        date.year(),
        date.month()
    );

    let response: SholidayFaboulResponse = client.get(url.as_str()).send()?.json()?;

    let last_work_day = response
        .days
        .iter()
        .rfind(|day| day.work_free_day == "Nej")
        .unwrap();

    let sholiday_day_date =
        NaiveDate::parse_from_str(last_work_day.date.clone().as_str(), "%Y-%m-%d")?;

    return Ok(sholiday_day_date);
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};

    use crate::last_day::is_last_workday;

    #[test]
    fn is_2020_10_31_last_work_day() {
        let date = Utc.with_ymd_and_hms(2020, 10, 31, 0, 0, 0);
        let is_last = is_last_workday(&date.unwrap()).expect("failed");

        assert!(!is_last)
    }

    #[test]
    fn is_2020_10_30_last_work_day() {
        let date = Utc.with_ymd_and_hms(2020, 10, 30, 0, 0, 0);
        let is_last = is_last_workday(&date.unwrap()).expect("failed");

        assert!(is_last)
    }

    #[test]
    fn is_2020_10_29_last_work_day() {
        let date = Utc.with_ymd_and_hms(2020, 10, 29, 0, 0, 0);
        let is_last = is_last_workday(&date.unwrap()).expect("failed");

        assert!(!is_last)
    }
}
