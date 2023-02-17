use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct Timer {
    pub name: String,
    #[serde(with = "duration_format")]
    pub interval: Duration,
    #[serde(with = "duration_format")]
    pub break_duration: Duration,
    pub notify: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub timers: Vec<Timer>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timers: vec![
                Timer {
                    name: "micro_break".to_string(),
                    interval: Duration::from_secs(22 * 60),
                    break_duration: Duration::from_secs(30),
                    notify: true,
                },
                Timer {
                    name: "break".to_string(),
                    interval: Duration::from_secs(55 * 60),
                    break_duration: Duration::from_secs(3 * 60),
                    notify: true,
                },
            ],
        }
    }
}

impl Config {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let config_str =
                fs::read_to_string(path).with_context(|| "Failed to read config file")?;
            Ok(serde_yaml::from_str::<Self>(&config_str)
                .with_context(|| "Failed to read config file")?)
        } else {
            let default_config = Config::default();
            let config_str = serde_yaml::to_string(&default_config).unwrap();
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| "Failed to create config directory")?;
            }
            fs::write(&path, &config_str).with_context(|| "Failed to write config file")?;
            info!("Created default config file at '{}'", path.display());
            Ok(default_config)
        }
    }
}

mod duration_format {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let total_secs = duration.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs - mins * 60;
        serializer.serialize_str(&(format!("{:02}:{:02}", mins, secs)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let center = str
            .find(":")
            .ok_or(Error::custom("missing ':' splitter on duration"))?;
        let mins = &str[..center]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse left integer: {}", e)))?;
        let secs = &str[center + 1..]
            .parse::<u64>()
            .map_err(|e| Error::custom(format!("failed to parse right integer: {}", e)))?;

        Ok(Duration::from_secs(mins * 60 + secs))
    }
}

mod duration_format_option {
    use super::duration_format;
    use serde::{de::Error, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(dur) => duration_format::serialize(dur, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match duration_format::deserialize(deserializer) {
            Ok(dur) => Ok(Some(dur)),
            Err(err) => Err(Error::custom(err)),
        }
    }
}
