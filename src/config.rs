use std::collections::HashMap;
use std::error::Error;

use rand::prelude::*;
use serde::Deserialize;
use serde_yaml;

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
    pub fn get_authors(&self) -> String {
        let names = self
            .intro
            .credits
            .names
            .iter()
            .map(|name| format!("\t- {}", name))
            .collect::<Vec<String>>()
            .join("\n");
        format!("{}\n\n{}", self.intro.credits.intro, names)
    }

    pub fn get_message(&self, context: &str) -> String {
        let beginning = Configuration::get_message_part(&self.time_report.beginning, context);
        let middle = Configuration::get_message_part(&self.time_report.middle, context);
        let end = Configuration::get_message_part(&self.time_report.end, context);
        format!("<!channel> {}\n{}\n{}", beginning, middle, end)
    }

    pub fn get_introduction(&self) -> String {
        let index = rand::rng().random_range(0..self.intro.greetings.len());
        let greeting = &self.intro.greetings[index];
        let features = self
            .intro
            .features
            .iter()
            .map(|feature| format!("\t- {}", feature))
            .collect::<Vec<String>>()
            .join("\n");

        // Sexiest line of code everest!
        format!(
            "{}\n\n{}\n\nSaker ni kan fråga (med `@joel-bot <kommando>`:\n{}",
            greeting, self.intro.about_me, features
        )
    }

    pub fn read() -> Result<Configuration, Box<dyn Error>> {
        let file = std::fs::File::open("config.yaml")?;
        let config: Configuration = serde_yaml::from_reader(file)?;
        Ok(config)
    }

    fn get_message_part(part: &Part, context: &str) -> String {
        let mut random = rand::rng();

        let part = match part.get(context) {
            None => part.get("general").unwrap(),
            Some(part) => part,
        };

        let index = random.random_range(0..part.len());

        let string = &part[index];

        string.clone()
    }
}
