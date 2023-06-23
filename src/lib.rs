use std::fs;
use serde::Deserialize;


#[derive(Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub templates_dir: String,
}

impl Settings {
    pub fn load_from_file(filename: &str) -> Result<Settings, Box<dyn std::error::Error>> {
        let settings_content = fs::read_to_string(filename)?;
        let settings: Settings = serde_json::from_str(&settings_content)?;
        Ok(settings)
    }
}

