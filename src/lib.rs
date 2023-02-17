pub mod config;
pub mod input_listener;

use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use std::{fmt::Debug, path::PathBuf, time::Duration};
use tracing::error;

#[derive(Debug, Clone, Decode, Encode)]
pub enum Message {
    List,
    Get(String),
    Reset(String),
    ResetAll,
    Inactive,
    Running,
}

#[derive(Debug, Clone, Decode, Encode)]
pub struct TimerInfo {
    pub elapsed: Duration,
    pub interval: Duration,
}

#[derive(Debug, Clone, Decode, Encode)]
pub enum Response {
    Ok,
    Duration(Duration),
    Timer(TimerInfo),
    List(Vec<(String, TimerInfo)>),
    Error(ResponseError),
}

#[derive(Debug, Clone, Decode, Encode)]
pub enum ResponseError {
    NotFound,
}

pub trait Serialization<T> {
    fn decode(bytes: &[u8]) -> Result<T>;
    fn encode(&self) -> Result<Vec<u8>>;
}

impl<T> Serialization<T> for T
where
    T: Decode + Encode,
{
    fn decode(bytes: &[u8]) -> Result<T> {
        bincode::decode_from_slice(bytes, bincode::config::standard())
            .map(|(t, _)| t)
            .with_context(|| "Failed to decode")
    }

    fn encode(&self) -> Result<Vec<u8>> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .with_context(|| "Failed to encode")
    }
}

const APP_NAME: &str = env!("CARGO_PKG_NAME");

pub fn config_path() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join(APP_NAME).join(APP_NAME).with_extension("toml"))
        .context("Couldn't find the config directory")
}

pub fn socket_path() -> PathBuf {
    PathBuf::from("/run").join(APP_NAME).with_extension("sock")
}

/// Sends a desktop notification
pub fn send_notification(title: String, description: String) {
    use notify_rust::*;

    std::thread::spawn(move || {
        if let Err(e) = Notification::new()
            .summary(&title)
            .body(&description)
            .appname(APP_NAME)
            .timeout(5)
            .show()
        {
            error!("Failed to send notification: {e}");
        }
    });
}
