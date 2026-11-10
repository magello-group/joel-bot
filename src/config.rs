use anyhow::{Context as _, Result, ensure};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use rand::seq::IndexedRandom;
use rand::{Rng, RngExt};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::occasion::Occasion;

/// Chance of using a month's messages, e.g. january or december, when there
/// are some. The rest is `general`, so the themed ones don't wear out.
const MONTH_THEME_CHANCE: f64 = 0.6;

/// Which messages of a part to choose from, a key under `time_report` in
/// `config.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Theme {
    General,
    Month(u32),
    Occasion(Occasion),
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(key: &str) -> Result<Self, String> {
        if key == "general" {
            return Ok(Theme::General);
        }
        // Compared as text so that e.g. "05" isn't accepted for May
        if let Some(month) = key
            .parse::<u32>()
            .ok()
            .filter(|month| (1..=12).contains(month) && month.to_string() == key)
        {
            return Ok(Theme::Month(month));
        }
        key.parse().map(Theme::Occasion).map_err(|()| {
            format!("unknown key '{key}', expected general, a month number or an occasion")
        })
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Theme::General => f.write_str("general"),
            Theme::Month(month) => write!(f, "{month}"),
            Theme::Occasion(occasion) => write!(f, "{occasion}"),
        }
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

type Part = HashMap<Theme, Vec<String>>;

#[derive(Deserialize, Debug)]
pub struct Configuration {
    intro: Intro,
    time_report: TimeReport,
}

#[derive(Deserialize, Debug)]
pub struct TimeReport {
    beginning: Part,
    middle: Part,
    end: Part,
}

#[derive(Deserialize, Debug)]
pub struct Intro {
    greetings: Vec<String>,
    about_me: String,
    features: Vec<String>,
    credits: Credits,
}

#[derive(Deserialize, Debug)]
pub struct Credits {
    intro: String,
    names: Vec<String>,
}

impl Configuration {
    #[must_use]
    pub fn get_authors(&self) -> String {
        let Credits { intro, names } = &self.intro.credits;
        let author_list = bullet_list(names);
        format!("{intro}\n\n{author_list}")
    }

    /// Picks the reminder text. An `occasion` (e.g. halloween) always decides
    /// the messages. Otherwise the `month`'s messages are used
    /// `MONTH_THEME_CHANCE` of the time, and everything else falls back to
    /// `general`.
    #[must_use]
    pub fn get_message(&self, occasion: Option<Occasion>, month: u32) -> String {
        self.get_message_with(occasion, month, &mut rand::rng())
    }

    fn get_message_with(
        &self,
        occasion: Option<Occasion>,
        month: u32,
        rng: &mut impl Rng,
    ) -> String {
        let theme = pick_theme(occasion, month, rng);
        let TimeReport {
            beginning,
            middle,
            end,
        } = &self.time_report;
        let beginning = Configuration::get_message_part(beginning, theme, rng);
        let middle = Configuration::get_message_part(middle, theme, rng);
        let end = Configuration::get_message_part(end, theme, rng);
        format!("<!channel> {beginning}\n{middle}\n{end}")
    }

    #[must_use]
    pub fn get_introduction(&self) -> String {
        let greeting = self
            .intro
            .greetings
            .choose(&mut rand::rng())
            .expect("no greetings configured");
        let about_me = &self.intro.about_me;
        let feature_list = bullet_list(&self.intro.features);

        // Sexiest line of code everest!
        format!(
            "{greeting}\n\n{about_me}\n\nSaker ni kan fråga (med `@joel-bot <kommando>`:\n{feature_list}"
        )
    }

    pub fn read() -> Result<Configuration> {
        let text = std::fs::read_to_string("config.toml").context("couldn't read config.toml")?;
        let config: Configuration = toml::from_str(&text).context("couldn't parse config.toml")?;
        config.validate()?;
        Ok(config)
    }

    /// Fails at startup, rather than on the last workday, if messages are missing.
    /// Keys that aren't recognised already fail when parsing.
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.intro.greetings.is_empty(),
            "config.toml: intro.greetings is empty"
        );
        let parts = [
            ("beginning", &self.time_report.beginning),
            ("middle", &self.time_report.middle),
            ("end", &self.time_report.end),
        ];
        for (name, part) in parts {
            for (theme, messages) in part {
                ensure!(
                    !messages.is_empty(),
                    "config.toml: time_report.{name}.{theme} is empty"
                );
            }
            ensure!(
                part.contains_key(&Theme::General),
                "config.toml: time_report.{name}.general needs at least one message"
            );
        }
        for (name, part) in parts {
            for occasion in Occasion::ALL {
                ensure!(
                    part.contains_key(&Theme::Occasion(occasion)),
                    "config.toml: time_report.{name}.{occasion} needs at least one message"
                );
            }
        }
        Ok(())
    }

    fn get_message_part(part: &Part, theme: Theme, rng: &mut impl Rng) -> String {
        part.get(&theme)
            .and_then(|messages| messages.choose(&mut *rng))
            .or_else(|| part.get(&Theme::General)?.choose(&mut *rng))
            .expect("no 'general' messages configured")
            .clone()
    }
}

fn pick_theme(occasion: Option<Occasion>, month: u32, rng: &mut impl Rng) -> Theme {
    if let Some(occasion) = occasion {
        Theme::Occasion(occasion)
    } else if rng.random_bool(MONTH_THEME_CHANCE) {
        Theme::Month(month)
    } else {
        Theme::General
    }
}

fn bullet_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("\t- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    const MINIMAL_CONFIG: &str = r#"
[intro]
greetings = ["Hej"]
about_me = ""
features = []

[intro.credits]
intro = ""
names = []

[time_report.beginning]
general = ["a"]

[time_report.middle]
general = []

[time_report.end]
general = ["c"]
"#;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(1)
    }

    #[test]
    fn config_toml_is_valid() {
        Configuration::read().unwrap();
    }

    #[test]
    fn month_keys_select_month_messages() {
        let config = Configuration::read().unwrap();
        let beginning = &config.time_report.beginning;
        let december = &beginning[&Theme::Month(12)];
        let message = Configuration::get_message_part(beginning, Theme::Month(12), &mut rng());
        assert!(december.contains(&message), "{message}");
    }

    #[test]
    fn occasion_keys_select_occasion_messages() {
        let config = Configuration::read().unwrap();
        for occasion in Occasion::ALL {
            let theme = Theme::Occasion(occasion);
            for part in [
                &config.time_report.beginning,
                &config.time_report.middle,
                &config.time_report.end,
            ] {
                let message = Configuration::get_message_part(part, theme, &mut rng());
                assert!(part[&theme].contains(&message), "{occasion}: {message}");
            }
        }
    }

    #[test]
    fn months_without_messages_use_general() {
        let config = Configuration::read().unwrap();
        let beginning = &config.time_report.beginning;
        let message = Configuration::get_message_part(beginning, Theme::Month(5), &mut rng());
        assert!(beginning[&Theme::General].contains(&message), "{message}");
    }

    #[test]
    fn occasions_always_win_and_months_mix_with_general() {
        let mut rng = rng();
        let mut seen = |occasion, month| -> HashSet<Theme> {
            (0..1000)
                .map(|_| pick_theme(occasion, month, &mut rng))
                .collect()
        };
        assert_eq!(
            seen(Some(Occasion::Halloween), 10),
            HashSet::from([Theme::Occasion(Occasion::Halloween)])
        );
        assert_eq!(
            seen(None, 12),
            HashSet::from([Theme::General, Theme::Month(12)])
        );
    }

    #[test]
    fn unknown_keys_are_rejected() {
        for key in ["halowen", "13", "0", "05", "General"] {
            let toml = MINIMAL_CONFIG.replace(
                "[time_report.beginning]\ngeneral",
                &format!("[time_report.beginning]\n{key}"),
            );
            let error = toml::from_str::<Configuration>(&toml)
                .unwrap_err()
                .to_string();
            assert!(error.contains("unknown key"), "{key}: {error}");
        }
    }

    #[test]
    fn empty_general_messages_are_rejected() {
        let config: Configuration = toml::from_str(MINIMAL_CONFIG).unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("time_report.middle"), "{error}");
    }

    #[test]
    fn missing_occasion_messages_are_rejected() {
        let toml = MINIMAL_CONFIG.replace("general = []", r#"general = ["b"]"#);
        let config: Configuration = toml::from_str(&toml).unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("time_report.beginning.easter"), "{error}");
    }
}
