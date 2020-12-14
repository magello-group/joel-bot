use std::collections::HashMap;
use std::error::Error;

use rand::prelude::*;
use serde::Deserialize;
use serde_yaml;

type Part = HashMap<String, Vec<String>>;

#[derive(Deserialize, Debug)]
pub struct Configuration {
    beginning: Part,
    middle: Part,
    end: Part,
}

impl Configuration {
    pub fn get_message(&self, context: &str) -> String {
        let beginning = Configuration::get_message_part(&self.beginning, context);
        let middle = Configuration::get_message_part(&self.middle, context);
        let end = Configuration::get_message_part(&self.end, context);
        format!("{}\n{}\n{}", beginning, middle, end)
    }

    pub fn read() -> Result<Configuration, Box<dyn Error>> {
        let file = std::fs::File::open("config.yaml")?;
        let config: Configuration = serde_yaml::from_reader(file)?;
        Ok(config)
    }

    fn get_message_part(part: &Part, context: &str) -> String {
        let mut random = rand::thread_rng();

        let part = match part.get(context) {
            None => part.get("general").unwrap(),
            Some(part) => part
        };

        let index = random.gen_range(0, part.len());

        let string = &part[index];

        string.clone()
    }
}

