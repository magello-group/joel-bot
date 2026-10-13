use anyhow::{Context, Result, ensure};
use std::collections::HashMap;

use rand::seq::IndexedRandom;
use serde::Deserialize;

type Part = HashMap<String, Vec<String>>;

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

    #[must_use]
    pub fn get_message(&self, context: &str) -> String {
        let beginning = Configuration::get_message_part(&self.time_report.beginning, context);
        let middle = Configuration::get_message_part(&self.time_report.middle, context);
        let end = Configuration::get_message_part(&self.time_report.end, context);
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
        let file = std::fs::File::open("config.yaml").context("couldn't open config.yaml")?;
        let config: Configuration =
            serde_norway::from_reader(file).context("couldn't parse config.yaml")?;
        config.validate()?;
        Ok(config)
    }

    /// Fails at startup, rather than on the last workday, if messages are missing.
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.intro.greetings.is_empty(),
            "config.yaml: intro.greetings is empty"
        );
        let parts = [
            ("beginning", &self.time_report.beginning),
            ("middle", &self.time_report.middle),
            ("end", &self.time_report.end),
        ];
        for (name, part) in parts {
            ensure!(
                part.get("general")
                    .is_some_and(|messages| !messages.is_empty()),
                "config.yaml: time_report.{name}.general needs at least one message"
            );
        }
        Ok(())
    }

    fn get_message_part(part: &Part, context: &str) -> String {
        part.get(context)
            .and_then(|messages| messages.choose(&mut rand::rng()))
            .or_else(|| part.get("general")?.choose(&mut rand::rng()))
            .expect("no 'general' messages configured")
            .clone()
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
    use super::*;

    #[test]
    fn config_yaml_is_valid() {
        Configuration::read().unwrap();
    }

    #[test]
    fn month_keys_select_month_messages() {
        let config = Configuration::read().unwrap();
        let december = &config.time_report.beginning["12"];
        let message = Configuration::get_message_part(&config.time_report.beginning, "12");
        assert!(december.contains(&message), "{message}");
    }

    #[test]
    fn empty_general_messages_are_rejected() {
        let yaml = r#"
intro:
  greetings: ["Hej"]
  about_me: ""
  features: []
  credits: { intro: "", names: [] }
time_report:
  beginning: { general: ["a"] }
  middle: { general: [] }
  end: { general: ["c"] }
"#;
        let config: Configuration = serde_norway::from_str(yaml).unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("time_report.middle.general"), "{error}");
    }
}
