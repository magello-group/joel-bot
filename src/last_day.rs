
use chrono::{Datelike, NaiveDate};
use reqwest::Client;
use serde::Deserialize;
use anyhow::Result;

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


pub async fn is_last_workday(date: &NaiveDate) -> Result<bool> {
    Ok(get_last_workday(date).await? == *date)
}

pub async fn get_last_workday(date: &NaiveDate) -> Result<NaiveDate> {
    let client = Client::new();
    let url = format!(
        "https://sholiday.faboul.se/dagar/v2.1/{}/{}",
        date.year(),
        date.month()
    );

    let response: SholidayFaboulResponse = client.get(url.as_str()).send().await?.json().await?;

    let last_work_day = response
        .days
        .iter()
        .rfind(|day| day.work_free_day == "Nej")
        .unwrap();

    let sholiday_day_date =
        NaiveDate::parse_from_str(last_work_day.date.clone().as_str(), "%Y-%m-%d")?;

    Ok(sholiday_day_date)
}

#[cfg(test)]
mod test {
    use chrono::{NaiveDate, TimeZone, Utc};
    use tokio;

    use super::{is_last_workday, get_last_workday};

    #[tokio::test]
    async fn is_2020_10_31_last_work_day() {
        let date = NaiveDate::from_ymd_opt(2020, 10, 31).unwrap();
        let is_last = is_last_workday(&date).await.expect("failed");

        assert!(!is_last)
    }

    #[tokio::test]
    async fn is_2020_10_30_last_work_day() {
        let date = NaiveDate::from_ymd_opt(2020, 10, 30).unwrap();
        let is_last = is_last_workday(&date).await.expect("failed");

        assert!(is_last)
    }

    #[tokio::test]
    async fn is_2020_10_29_last_work_day() {
        let date = NaiveDate::from_ymd_opt(2020, 10, 29).unwrap();
        let is_last = is_last_workday(&date).await.expect("failed");

        assert!(!is_last)
    }

    #[tokio::test]
    async fn test_get_last_workday() {
        let date = NaiveDate::from_ymd_opt(2020, 10, 31).unwrap();
        let last_workday = get_last_workday(&date).await.expect("failed");

        // 2020-10-30 was a Friday, and therefore the last workday of October 2020
        let expected_last_workday = Utc.with_ymd_and_hms(2020, 10, 30, 0, 0, 0).unwrap().date_naive();
        assert_eq!(last_workday, expected_last_workday);
    }
}
