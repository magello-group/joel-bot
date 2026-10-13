use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use chrono_tz::{Europe::Stockholm, Tz};
use reqwest::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::holidays::last_workday_local;

const API_ATTEMPTS: u32 = 3;
/// Timeout per API attempt. The API normally answers in well under a second.
const API_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(2);
/// Pause before retry n is n times this. 3 attempts of 2s plus 0.5s + 1s of
/// back-off is 7.5s, which fits within `API_BUDGET`.
const API_BACKOFF: Duration = Duration::from_millis(500);
/// Total time to wait on the API, retries included, before using the local
/// calendar. Keeps answers to Slack users quick even when the API hangs.
const API_BUDGET: Duration = Duration::from_secs(8);

#[derive(Deserialize)]
struct SholidayFaboulResponse<Day> {
    #[serde(alias = "dagar")]
    days: Vec<Day>,
}

#[derive(Deserialize)]
pub(crate) struct SholidayFaboulDay {
    #[serde(alias = "datum")]
    pub(crate) date: NaiveDate,
    #[serde(alias = "arbetsfri dag")]
    work_free_day: String,
}

impl SholidayFaboulDay {
    pub(crate) fn is_work_free(&self) -> bool {
        self.work_free_day == "Ja"
    }
}

/// The last day in `days` that is not work free.
pub(crate) fn last_workday_in<'a>(
    mut days: impl DoubleEndedIterator<Item = &'a SholidayFaboulDay>,
) -> Option<NaiveDate> {
    days.rfind(|day| !day.is_work_free()).map(|day| day.date)
}

/// Fetches days from the holiday API, e.g. `path` = "2020/10" or "2020".
/// Retries a few times since the API is occasionally flaky.
pub(crate) async fn fetch_days<Day: DeserializeOwned>(
    client: &Client,
    path: &str,
    attempt_timeout: Duration,
) -> Result<Vec<Day>> {
    let url = format!("https://sholiday.faboul.se/dagar/v2.1/{path}");
    let mut attempt = 1;
    loop {
        match fetch_days_once(client, &url, attempt_timeout).await {
            Ok(days) => return Ok(days),
            Err(error) if attempt < API_ATTEMPTS && is_transient(&error) => {
                eprintln!("holiday API attempt {attempt} failed: {error:#}");
                tokio::time::sleep(API_BACKOFF * attempt).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Everything is retried except client errors (other than 429), which a retry
/// can't fix. A dropped connection can surface as a decode error, so those are
/// retried too.
fn is_transient(error: &anyhow::Error) -> bool {
    let client_error = error
        .downcast_ref::<reqwest::Error>()
        .and_then(reqwest::Error::status)
        .is_some_and(|status| {
            status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS
        });
    !client_error
}

async fn fetch_days_once<Day: DeserializeOwned>(
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<Vec<Day>> {
    let response: SholidayFaboulResponse<Day> = client
        .get(url)
        .timeout(timeout)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(response.days)
}

async fn get_last_workday_from_api(client: &Client, date: &NaiveDate) -> Result<NaiveDate> {
    let year = date.year();
    let month = date.month();
    let days: Vec<SholidayFaboulDay> =
        fetch_days(client, &format!("{year}/{month}"), API_ATTEMPT_TIMEOUT).await?;
    last_workday_in(days.iter()).with_context(|| format!("no workday found in {year}-{month:02}"))
}

/// The current time in Sweden.
#[must_use]
pub fn now() -> DateTime<Tz> {
    Utc::now().with_timezone(&Stockholm)
}

/// Today's date in Sweden.
#[must_use]
pub fn today() -> NaiveDate {
    now().date_naive()
}

pub async fn is_last_workday(date: &NaiveDate) -> Result<bool> {
    Ok(get_last_workday(date).await? == *date)
}

/// How long API answers are cached, so corrections reach the long-running server.
const API_TTL: Duration = Duration::from_hours(24);
/// How long a locally calculated fallback is cached before the API is tried again.
const FALLBACK_TTL: Duration = Duration::from_mins(10);

struct CacheEntry {
    last_workday: NaiveDate,
    expires: Instant,
}

/// Last workday of the month of `date`. Prefers the holiday API, falls back to
/// the local calendar if the API is unavailable. Results are cached per month.
pub async fn get_last_workday(date: &NaiveDate) -> Result<NaiveDate> {
    static CACHE: LazyLock<Mutex<HashMap<(i32, u32), CacheEntry>>> =
        LazyLock::new(Default::default);
    let key = (date.year(), date.month());

    if let Some(entry) = CACHE.lock().unwrap().get(&key)
        && Instant::now() < entry.expires
    {
        return Ok(entry.last_workday);
    }

    let api_lookup = get_last_workday_from_api(&crate::http::CLIENT, date);
    let api_result = tokio::time::timeout(API_BUDGET, api_lookup)
        .await
        .unwrap_or_else(|_| Err(anyhow!("no answer within {API_BUDGET:?}")));
    let (last_workday, time_to_live) = match api_result {
        Ok(last_workday) => (last_workday, API_TTL),
        Err(error) => {
            eprintln!("holiday API failed ({error:#}), using local calendar");
            let last_workday = last_workday_local(date.year(), date.month())
                .context("couldn't calculate last workday locally")?;
            (last_workday, FALLBACK_TTL)
        }
    };
    let entry = CacheEntry {
        last_workday,
        expires: Instant::now() + time_to_live,
    };
    CACHE.lock().unwrap().insert(key, entry);
    Ok(last_workday)
}

/// The next day you need to time report: this month's last workday, or next
/// month's if this month's has already passed.
pub async fn next_time_report_day(today: &NaiveDate) -> Result<NaiveDate> {
    next_time_report_day_with(today, get_last_workday).await
}

/// Looks up next month only when this month's last workday has passed. When
/// the local calendar says that is likely, both months are looked up
/// concurrently so a slow API is waited on once.
async fn next_time_report_day_with(
    today: &NaiveDate,
    last_workday_of: impl AsyncFn(&NaiveDate) -> Result<NaiveDate>,
) -> Result<NaiveDate> {
    let first_of_next_month = today
        .with_day(1)
        .and_then(|first| first.checked_add_months(Months::new(1)))
        .context("date out of range")?;
    let rollover_likely = last_workday_local(today.year(), today.month())
        .is_some_and(|last_workday| last_workday < *today);

    let (this_month, next_month) = if rollover_likely {
        let (this_month, next_month) = tokio::join!(
            last_workday_of(today),
            last_workday_of(&first_of_next_month)
        );
        (this_month?, Some(next_month))
    } else {
        (last_workday_of(today).await?, None)
    };

    if this_month >= *today {
        return Ok(this_month);
    }
    match next_month {
        Some(next_month) => next_month,
        None => last_workday_of(&first_of_next_month).await,
    }
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;

    use super::{get_last_workday_from_api, next_time_report_day_with};
    use crate::holidays::last_workday_local;
    use anyhow::Context;
    use chrono::Datelike;
    use reqwest::Client;

    #[tokio::test]
    #[ignore = "calls the live holiday API, run with `cargo test -- --ignored`"]
    async fn last_workday_of_october_2020() {
        let date = |day| NaiveDate::from_ymd_opt(2020, 10, day).unwrap();
        let client = Client::new();
        let last_workday = get_last_workday_from_api(&client, &date(31))
            .await
            .expect("failed");

        // 2020-10-30 was a Friday, and therefore the last workday of October 2020
        assert_eq!(last_workday, date(30));
    }

    #[tokio::test]
    #[ignore = "calls the live holiday API, run with `cargo test -- --ignored`"]
    async fn rolls_over_to_next_month_after_last_workday() {
        // Uses the API directly, so the local fallback can't hide an API outage
        let next = async |today| {
            let client = Client::new();
            let from_api = async |date: &NaiveDate| get_last_workday_from_api(&client, date).await;
            next_time_report_day_with(&today, from_api).await.unwrap()
        };
        let date = |month, day| NaiveDate::from_ymd_opt(2026, month, day).unwrap();
        // 2026-10-30 is the last workday of October, the 31st is a Saturday
        assert_eq!(next(date(10, 30)).await, date(10, 30));
        assert_eq!(next(date(10, 31)).await, date(11, 30));
    }

    #[tokio::test]
    async fn rolls_over_with_local_calendar() {
        let local = async |date: &NaiveDate| {
            last_workday_local(date.year(), date.month()).context("no workday")
        };
        let date = |month, day| NaiveDate::from_ymd_opt(2026, month, day).unwrap();
        let next = async |today| next_time_report_day_with(&today, local).await.unwrap();
        assert_eq!(next(date(10, 13)).await, date(10, 30));
        assert_eq!(next(date(10, 30)).await, date(10, 30));
        assert_eq!(next(date(10, 31)).await, date(11, 30));
        // Rolls over the year too: 2026-12-31 is nyårsafton
        assert_eq!(
            next(date(12, 31)).await,
            NaiveDate::from_ymd_opt(2027, 1, 29).unwrap()
        );
    }
}
