pub mod cli;
pub mod config;
pub mod input_listener;
pub mod msg;
pub mod socket;

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::error;

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const DAEMON_NAME: &str = "moved";
pub const ACTIVITY_DAEMON_NAME: &str = "actived";

pub fn daemon_socket() -> PathBuf {
    dirs::runtime_dir()
        .expect("No runtime directory found!")
        .join(APP_NAME)
        .join(DAEMON_NAME)
}

pub fn activity_daemon_socket() -> PathBuf {
    PathBuf::from("/run")
        .join(APP_NAME)
        .join(ACTIVITY_DAEMON_NAME)
}

pub fn config_path() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join(APP_NAME).join(APP_NAME).with_extension("toml"))
        .context("Couldn't find the config directory")
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
