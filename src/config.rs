use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path, time::Duration};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Timer {
    pub name: String,
    #[serde(with = "mmss_format")]
    pub interval: Duration,
    #[serde(
        default,
        with = "mmss_format_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub suggested: Option<Duration>,
    pub duration: Option<Duration>,
    pub notify: bool,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub timers: Vec<Timer>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timers: vec![
                Timer {
                    name: "move".to_string(),
                    interval: Duration::from_secs(25 * 60),
                    suggested: None,
                    duration: None,
                    notify: false,
                },
                Timer {
                    name: "break".to_string(),
                    interval: Duration::from_secs(2 * 60 * 60),
                    suggested: Some(Duration::from_secs(55 * 60)),
                    duration: Some(Duration::from_secs(10 * 60)),
                    notify: true,
                },
            ],
        }
    }
}

impl Config {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            let config_str =
                fs::read_to_string(path).with_context(|| "Failed to read configuration file")?;
            Ok(
                toml::from_str::<Self>(&config_str)
                    .with_context(|| "Failed to read configuration file")?,
            )
        } else {
            info!("No config file found, using default configuration");
            Ok(Config::default())
        }
    }
}

mod mmss_format {
    use serde::{de::Error, Deserialize, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let center = str
            .find(':')
            .ok_or_else(|| Error::custom("missing ':' splitter on duration"))?;
        let mins = &str[..center]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse left integer: {}", e)))?;
        let secs = &str[center + 1..]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse right integer: {}", e)))?;

        Ok(Duration::from_secs(mins * 60 + secs))
    }
}

mod mmss_format_opt {
    use super::mmss_format;
    use serde::{de::Error, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match mmss_format::deserialize(deserializer) {
            Ok(dur) => Ok(Some(dur)),
            Err(err) => Err(Error::custom(err)),
        }
    }
}
