//! Themes for the last-workday reminder, e.g. halloween or easter.
//!
//! Each occasion has messages under its own key in `config.yaml`.

use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};

use crate::holidays::easter_sunday;

/// How many days before Easter Sunday the reminder still gets the easter theme.
const EASTER_DAYS_BEFORE: i64 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Occasion {
    Easter,
    Valborg,
    Midsommar,
    Semester,
    Kraftskiva,
    Halloween,
}

impl Occasion {
    pub const ALL: [Occasion; 6] = [
        Occasion::Easter,
        Occasion::Valborg,
        Occasion::Midsommar,
        Occasion::Semester,
        Occasion::Kraftskiva,
        Occasion::Halloween,
    ];

    /// The key used in `config.yaml`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Occasion::Easter => "easter",
            Occasion::Valborg => "valborg",
            Occasion::Midsommar => "midsommar",
            Occasion::Semester => "semester",
            Occasion::Kraftskiva => "kraftskiva",
            Occasion::Halloween => "halloween",
        }
    }
}

impl fmt::Display for Occasion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl FromStr for Occasion {
    type Err = ();

    fn from_str(key: &str) -> Result<Self, ()> {
        Occasion::ALL
            .into_iter()
            .find(|occasion| occasion.key() == key)
            .ok_or(())
    }
}

/// The theme for the reminder sent on `last_workday`, the last workday of its
/// month. Easter depends on the exact date, the rest on the month, since the
/// last workday of April, June, July, August and October always lands close
/// enough to the occasion. Not meant for other dates.
#[must_use]
pub fn occasion(last_workday: NaiveDate) -> Option<Occasion> {
    let days_until_easter = (easter_sunday(last_workday.year()) - last_workday).num_days();
    if (0..=EASTER_DAYS_BEFORE).contains(&days_until_easter) {
        return Some(Occasion::Easter);
    }
    match last_workday.month() {
        4 => Some(Occasion::Valborg),
        6 => Some(Occasion::Midsommar),
        7 => Some(Occasion::Semester),
        8 => Some(Occasion::Kraftskiva),
        10 => Some(Occasion::Halloween),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;
    use crate::holidays::last_workday_local;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn occasions_of_last_workdays() {
        assert_eq!(occasion(date(2026, 10, 30)), Some(Occasion::Halloween));
        assert_eq!(occasion(date(2025, 10, 31)), Some(Occasion::Halloween));
        assert_eq!(occasion(date(2026, 4, 30)), Some(Occasion::Valborg));
        assert_eq!(occasion(date(2026, 6, 30)), Some(Occasion::Midsommar));
        assert_eq!(occasion(date(2026, 7, 31)), Some(Occasion::Semester));
        assert_eq!(occasion(date(2026, 8, 31)), Some(Occasion::Kraftskiva));
        assert_eq!(occasion(date(2026, 11, 30)), None);
        assert_eq!(occasion(date(2026, 12, 30)), None);
    }

    #[test]
    fn easter_window() {
        // Easter Sunday 2026 is 5 April
        assert_eq!(occasion(date(2026, 4, 5)), Some(Occasion::Easter));
        assert_eq!(occasion(date(2026, 3, 31)), Some(Occasion::Easter));
        assert_eq!(occasion(date(2026, 3, 29)), Some(Occasion::Easter));
        // 8 days before is outside the window
        assert_eq!(occasion(date(2026, 3, 28)), None);
        // 2024-03-28 was the Thursday before långfredagen
        assert_eq!(occasion(date(2024, 3, 28)), Some(Occasion::Easter));
        // Easter 2027 is 28 March, so the last workday of March is after it
        assert_eq!(occasion(date(2027, 3, 29)), None);
        assert_eq!(occasion(date(2027, 3, 31)), None);
        assert_eq!(occasion(date(2025, 3, 31)), None);
    }

    #[test]
    fn keys_round_trip() {
        for occasion in Occasion::ALL {
            assert_eq!(occasion.key().parse(), Ok(occasion));
        }
        assert!("halowen".parse::<Occasion>().is_err());
    }

    /// Every occasion can actually occur, so none is dead config.
    #[test]
    fn every_occasion_is_reachable() {
        let reached: HashSet<_> = (2024..=2040)
            .flat_map(|year| (1..=12).map(move |month| (year, month)))
            .filter_map(|(year, month)| last_workday_local(year, month))
            .filter_map(occasion)
            .collect();
        assert_eq!(reached, HashSet::from(Occasion::ALL));
    }
}
