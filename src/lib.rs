pub mod config;
pub mod input_listener;

use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use std::{fmt::Debug, path::PathBuf, time::Duration};
use tracing::error;

#[derive(Debug, Clone, clap::Subcommand, Decode, Encode)]
pub enum Command {
    /// Get a list of values from all timers
    List,
    /// Get the value of a specific timer
    Get { name: String },
    /// Reset a specific timer
    Reset { name: String },
    /// Reset all timers
    ResetAll,
    /// Get duration of inactivity
    Inactive,
    /// Get duration of running
    Running,
}

#[derive(Debug, Clone, Decode, Encode)]
pub enum Response {
    Ok,
    Duration(Duration),
    List(Vec<(String, Duration)>),
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
        .map(|d| d.join(APP_NAME).join(APP_NAME).with_extension("yaml"))
        .context("Couldn't find the config directory")
}

pub fn socket_path() -> Result<PathBuf> {
    dirs::runtime_dir()
        .map(|d| d.join(APP_NAME).join(APP_NAME).with_extension("sock"))
        .context("Couldn't find the runtime directory")
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
