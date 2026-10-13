//! Local calculation of Swedish work-free days.
//!
//! Used as a fallback when the holiday API is unavailable. Mirrors what the
//! API reports as "arbetsfri dag": weekends, public holidays and the de facto
//! work-free eves (midsommarafton, julafton and nyårsafton).

use chrono::{Datelike, Days, NaiveDate, Weekday};

/// Easter Sunday for the given year (anonymous Gregorian algorithm).
#[expect(
    clippy::many_single_char_names,
    reason = "variable names follow the published algorithm"
)]
fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month.cast_unsigned(), day.cast_unsigned())
        .expect("valid easter date")
}

/// Midsommarafton is the Friday between 19 and 25 June.
fn midsummer_eve(year: i32) -> NaiveDate {
    (19..=25)
        .filter_map(|day| NaiveDate::from_ymd_opt(year, 6, day))
        .find(|date| date.weekday() == Weekday::Fri)
        .expect("there is always a Friday between 19 and 25 June")
}

/// Weekday holidays and eves that are work free. Days that always fall on a
/// weekend (påskdagen, pingstdagen, alla helgons dag) are covered by the
/// weekend check.
fn work_free_dates(year: i32) -> Vec<NaiveDate> {
    let fixed = [
        (1, 1),   // nyårsdagen
        (1, 6),   // trettondedag jul
        (5, 1),   // första maj
        (6, 6),   // Sveriges nationaldag
        (12, 24), // julafton
        (12, 25), // juldagen
        (12, 26), // annandag jul
        (12, 31), // nyårsafton
    ];
    let easter = easter_sunday(year);
    let moveable = [
        easter - Days::new(2),  // långfredagen
        easter + Days::new(1),  // annandag påsk
        easter + Days::new(39), // Kristi himmelsfärdsdag
        midsummer_eve(year),    // midsommarafton
    ];

    fixed
        .iter()
        .filter_map(|&(month, day)| NaiveDate::from_ymd_opt(year, month, day))
        .chain(moveable)
        .collect()
}

fn is_weekend_or_in(date: NaiveDate, work_free_dates: &[NaiveDate]) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun) || work_free_dates.contains(&date)
}

/// Whether `date` is a weekend, public holiday or work-free eve.
#[must_use]
pub fn is_work_free(date: NaiveDate) -> bool {
    is_weekend_or_in(date, &work_free_dates(date.year()))
}

/// The last day of the given month that is not work free.
#[must_use]
pub fn last_workday_local(year: i32, month: u32) -> Option<NaiveDate> {
    let work_free_dates = work_free_dates(year);
    NaiveDate::from_ymd_opt(year, month, 1)?
        .iter_days()
        .take_while(|date| date.month() == month)
        .filter(|&date| !is_weekend_or_in(date, &work_free_dates))
        .last()
}

#[cfg(test)]
mod test {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn easter() {
        assert_eq!(easter_sunday(2021), date(2021, 4, 4));
        assert_eq!(easter_sunday(2024), date(2024, 3, 31));
        assert_eq!(easter_sunday(2026), date(2026, 4, 5));
    }

    #[test]
    fn known_work_free_days() {
        assert!(is_work_free(date(2021, 4, 5))); // annandag påsk
        assert!(is_work_free(date(2026, 6, 19))); // midsommarafton
        assert!(is_work_free(date(2026, 5, 14))); // Kristi himmelsfärdsdag
        assert!(!is_work_free(date(2026, 10, 13)));
    }

    #[test]
    fn known_last_workdays() {
        assert_eq!(last_workday_local(2020, 10), Some(date(2020, 10, 30)));
        assert_eq!(last_workday_local(2026, 12), Some(date(2026, 12, 30)));
        // 2024-03-29 was långfredagen
        assert_eq!(last_workday_local(2024, 3), Some(date(2024, 3, 28)));
    }

    /// Days where the API is known to be wrong. 2022-06-06 was both
    /// nationaldagen and annandag pingst, and the API only kept the latter
    /// (which is no longer a holiday).
    const KNOWN_API_ERRORS: [NaiveDate; 1] = [NaiveDate::from_ymd_opt(2022, 6, 6).unwrap()];

    /// Validates the local calendar against the live holiday API, day by day.
    #[tokio::test]
    #[ignore = "calls the live holiday API, run with `cargo test -- --ignored`"]
    async fn live_validation() {
        use crate::last_day::{SholidayFaboulDay, fetch_days, last_workday_in};
        use serde::Deserialize;
        use std::time::Duration;

        /// The API's day plus fields that only help explain a mismatch.
        #[derive(Deserialize)]
        struct DiagnosticDay {
            #[serde(flatten)]
            day: SholidayFaboulDay,
            #[serde(rename = "röd dag")]
            red_day: Option<String>,
            #[serde(rename = "helgdag")]
            holiday: Option<String>,
        }

        let client = reqwest::Client::new();

        let mut mismatches = Vec::new();
        for year in 2020..=2030 {
            let local_work_free_dates = work_free_dates(year);
            let diagnostic_days: Vec<DiagnosticDay> =
                fetch_days(&client, &year.to_string(), Duration::from_secs(30))
                    .await
                    .unwrap_or_else(|error| panic!("couldn't fetch {year} from API: {error:#}"));

            for DiagnosticDay {
                day,
                red_day,
                holiday,
            } in &diagnostic_days
            {
                let date = day.date;
                let api_work_free = day.is_work_free();
                let local_work_free = is_weekend_or_in(date, &local_work_free_dates);
                if local_work_free != api_work_free && !KNOWN_API_ERRORS.contains(&date) {
                    mismatches.push(format!(
                        "{date}: api work free = {api_work_free}, local = {local_work_free}, röd dag = {red_day:?}, helgdag = {holiday:?}"
                    ));
                }
            }

            for month in 1..=12 {
                let days_in_month = diagnostic_days
                    .iter()
                    .map(|diagnostic_day| &diagnostic_day.day)
                    .filter(|day| day.date.month() == month);
                let api_last = last_workday_in(days_in_month);
                let local_last = last_workday_local(year, month);
                if api_last != local_last {
                    mismatches.push(format!(
                        "{year}-{month:02}: api last workday = {api_last:?}, local = {local_last:?}"
                    ));
                }
            }
        }

        let mismatch_report = mismatches.join("\n");
        assert!(
            mismatches.is_empty(),
            "local calendar differs from API:\n{mismatch_report}"
        );
    }
}
