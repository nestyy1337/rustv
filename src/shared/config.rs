use anyhow::Result;
use serde_aux::field_attributes::deserialize_number_from_string;
use std::{env, str::FromStr, sync::LazyLock};

use config::Config;
use dotenv::dotenv;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseSettings {
    pub database_path: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiKeys {
    pub tmdb: String,
    pub jackett: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub apikeys: ApiKeys,
    #[cfg(feature = "s3")]
    pub aws: AwsConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AwsConfig {
    pub region: String,
    pub bucket: String,
}

enum Environment {
    Production,
    Development,
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PROD" => Ok(Environment::Production),
            "DEV" => Ok(Environment::Development),
            _ => Err("Cannot match Env var to environment setting".to_string()),
        }
    }
}

pub static SETTINGS: LazyLock<Settings> =
    LazyLock::new(|| Settings::new().expect("failed to initalize settings"));

impl Settings {
    pub fn new() -> Result<Self> {
        dotenv().ok();
        let app_env = env::vars()
            .find_map(|(k, v)| {
                if k == "APP_ENVIRONMENT" {
                    Some(v)
                } else {
                    None
                }
            })
            .unwrap_or("DEV".to_string());

        let env_var = Environment::from_str(&app_env).unwrap_or(Environment::Development);
        let cfg_path = match env_var {
            Environment::Production => "./config/prod.yml",
            Environment::Development => "./config/local.yml",
        };
        let settings = Config::builder()
            .add_source(config::File::with_name(cfg_path))
            .add_source(config::Environment::with_prefix("APP").separator("__"))
            .build()
            .expect("failed to create a config");

        let settings_struct = settings
            .try_deserialize::<Settings>()
            .expect("failed to deserialize into settings struct");

        Ok(settings_struct)
    }
}
