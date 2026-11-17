//! Local calculation of Swedish work-free days.
//!
//! Used as a fallback when the holiday API is unavailable. Mirrors what the
//! API reports as "arbetsfri dag": weekends, public holidays and the de facto
//! work-free eves (midsommarafton, julafton and nyårsafton).

use jiff::ToSpan;
use jiff::civil::{Date, Weekday, date};

/// Easter Sunday for the given year (anonymous Gregorian algorithm).
#[expect(
    clippy::many_single_char_names,
    reason = "variable names follow the published algorithm"
)]
pub(crate) fn easter_sunday(year: i16) -> Date {
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
    // The published algorithm turns `h + l - 7 * m + 114` into a month and
    // day. That is the same as counting `h + l - 7 * m` days from 22 March.
    date(year, 3, 22) + (h + l - 7 * m).days()
}

/// Midsommarafton is the Friday between 19 and 25 June, i.e. the first
/// Friday after 18 June (`nth_weekday` doesn't count the date itself).
fn midsummer_eve(year: i16) -> Date {
    date(year, 6, 18)
        .nth_weekday(1, Weekday::Friday)
        .expect("midsommarafton is within jiff's supported range")
}

/// Weekday holidays and eves that are work free. Days that always fall on a
/// weekend (påskdagen, pingstdagen, alla helgons dag) are covered by the
/// weekend check. An invalid entry in `fixed` panics on purpose, rather than
/// quietly dropping the holiday; `fixed_dates_are_valid` catches it in tests.
fn work_free_dates(year: i16) -> Vec<Date> {
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
        easter - 2.days(),   // långfredagen
        easter + 1.day(),    // annandag påsk
        easter + 39.days(),  // Kristi himmelsfärdsdag
        midsummer_eve(year), // midsommarafton
    ];

    fixed
        .iter()
        .map(|&(month, day)| date(year, month, day))
        .chain(moveable)
        .collect()
}

fn is_weekend_or_in(day: Date, work_free_dates: &[Date]) -> bool {
    matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) || work_free_dates.contains(&day)
}

/// Whether `day` is a weekend, public holiday or work-free eve.
#[must_use]
pub fn is_work_free(day: Date) -> bool {
    is_weekend_or_in(day, &work_free_dates(day.year()))
}

/// The last day that is not work free in the month of `day`.
#[must_use]
pub fn last_workday_local(day: Date) -> Date {
    let work_free_dates = work_free_dates(day.year());
    day.last_of_month()
        .series(-1.day())
        .take_while(|other| other.month() == day.month())
        .find(|&day| !is_weekend_or_in(day, &work_free_dates))
        .expect("every month has weekdays that aren't holidays")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn easter() {
        assert_eq!(easter_sunday(2021), date(2021, 4, 4));
        assert_eq!(easter_sunday(2024), date(2024, 3, 31));
        assert_eq!(easter_sunday(2026), date(2026, 4, 5));
        // Earliest possible Easter, 0 days after 22 March
        assert_eq!(easter_sunday(2285), date(2285, 3, 22));
        // Latest possible Easter, 34 days after 22 March
        assert_eq!(easter_sunday(2038), date(2038, 4, 25));
        assert_eq!(easter_sunday(2008), date(2008, 3, 23));
    }

    /// `work_free_dates` panics on an invalid fixed date. Several years are
    /// checked so that e.g. a 29 February entry fails too.
    #[test]
    fn fixed_dates_are_valid() {
        for year in 2000..=2100 {
            work_free_dates(year);
        }
    }

    #[test]
    fn midsummer_eve_is_friday_between_19_and_25_june() {
        assert_eq!(midsummer_eve(2026), date(2026, 6, 19));
        assert_eq!(midsummer_eve(2027), date(2027, 6, 25));
        assert_eq!(midsummer_eve(2025), date(2025, 6, 20));
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
        assert_eq!(last_workday_local(date(2020, 10, 1)), date(2020, 10, 30));
        assert_eq!(last_workday_local(date(2026, 12, 31)), date(2026, 12, 30));
        // 2024-03-29 was långfredagen
        assert_eq!(last_workday_local(date(2024, 3, 15)), date(2024, 3, 28));
    }

    /// Days where the API is known to be wrong. 2022-06-06 was both
    /// nationaldagen and annandag pingst, and the API only kept the latter
    /// (which is no longer a holiday).
    const KNOWN_API_ERRORS: [Date; 1] = [date(2022, 6, 6)];

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

        let client = crate::http::client();

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
                let local_last = last_workday_local(date(year, month, 1));
                if api_last != Some(local_last) {
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
