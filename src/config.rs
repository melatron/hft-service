use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LogConfig {
    pub level: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub log: LogConfig,
}

impl Config {
    #[allow(dead_code)]
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Return a boxed trait object for flexibility
        Figment::new()
            .merge(Toml::file("Config.toml"))
            .merge(Env::prefixed("APP_"))
            .extract()
            .map_err(|e| e.into()) // Convert figment::Error into Box<dyn Error>
    }
}
