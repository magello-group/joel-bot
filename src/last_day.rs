use std::error::Error;

use chrono::{Datelike, DateTime, NaiveDate, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::slack::SlackClient;

#[derive(Deserialize)]
struct SholidayFaboulResponse {
    #[serde(alias = "startdatum")]
    start_date: String,
    #[serde(alias = "slutdatum")]
    end_date: String,
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
    let client = Client::new();
    let url = format!("https://sholiday.faboul.se/dagar/v2.1/{}/{}", date.year(), date.month());

    let response: SholidayFaboulResponse = client.get(url.as_str())
        .send()?
        .json()?;

    let last_work_day = response.days
        .iter()
        .rfind(|day| day.work_free_day == "Nej")
        .unwrap();

    let sholiday_day_date = NaiveDate::parse_from_str(last_work_day.date.clone().as_str(), "%Y-%m-%d")?;

    return Ok(sholiday_day_date == date.naive_utc().date())
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};

    use crate::last_day::is_last_workday;

    #[test]
    fn is_2020_10_31_last_work_day() {
        let date = Utc.ymd(2020, 10, 31).and_hms(0, 0, 0);
        let is_last = is_last_workday(&date).expect("failed");

        assert!(!is_last)
    }

    #[test]
    fn is_2020_10_30_last_work_day() {
        let date = Utc.ymd(2020, 10, 30).and_hms(0, 0, 0);
        let is_last = is_last_workday(&date).expect("failed");

        assert!(is_last)
    }

    #[test]
    fn is_2020_10_29_last_work_day() {
        let date = Utc.ymd(2020, 10, 29).and_hms(0, 0, 0);
        let is_last = is_last_workday(&date).expect("failed");

        assert!(!is_last)
    }
}
