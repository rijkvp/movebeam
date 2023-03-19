use crate::cli::CliCommand;
use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use std::{fmt::Debug, time::Duration};

#[derive(Debug, Clone, Decode, Encode)]
pub enum ActivityMessage {
    Get,
}

#[derive(Debug, Clone, Decode, Encode)]
pub enum Message {
    List,
    Get(String),
    Reset(String),
    ResetAll,
    Running,
}

impl Into<Message> for CliCommand {
    fn into(self) -> Message {
        match self {
            CliCommand::List => Message::List,
            CliCommand::Get { name } | CliCommand::Bar { name, .. } => Message::Get(name),
            CliCommand::Reset { name } => Message::Reset(name),
            CliCommand::ResetAll => Message::ResetAll,
            CliCommand::Running => Message::Running,
        }
    }
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

pub trait Encoding<T> {
    fn decode(bytes: &[u8]) -> Result<T>;
    fn encode(&self) -> Result<Vec<u8>>;
}

impl<T> Encoding<T> for T
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
