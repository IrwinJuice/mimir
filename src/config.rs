use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub service: ServiceConfig,
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let cfg = Config::builder()
            .add_source(File::with_name("Mimir").required(false))
            .set_default("service.port", 42000)?
            .build()?;

        cfg.try_deserialize()
    }
}

