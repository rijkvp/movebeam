use anyhow::{Context, Result};
use clap::Parser;
use movebeam::{Message, Response, Serialization};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    time::Duration,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// List of information from all timers
    List,
    /// Get the information of a specific timer
    Get { name: String },
    /// Get the information of a specific timer
    Bar {
        name: String,
        #[clap(short, long, default_value_t = 16)]
        size: usize,
        #[clap(short, long, default_value_t = String::from("█"))]
        fill: String,
        #[clap(short, long, default_value_t = String::from(" "))]
        empty: String,
        #[clap(short, long, default_value_t = String::from("["))]
        left: String,
        #[clap(short, long, default_value_t = String::from("]"))]
        right: String,
    },
    /// Reset a specific timer
    Reset { name: String },
    /// Reset all timers
    ResetAll,
    /// Get duration of inactivity
    Inactive,
    /// Get duration of running
    Running,
}

impl Into<Message> for Command {
    fn into(self) -> Message {
        match self {
            Command::List => Message::List,
            Command::Get { name } | Command::Bar { name, .. } => Message::Get(name),
            Command::Reset { name } => Message::Reset(name),
            Command::ResetAll => Message::ResetAll,
            Command::Inactive => Message::Inactive,
            Command::Running => Message::Running,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let socket_path = movebeam::socket_path();
    let mut stream =
        UnixStream::connect(socket_path).with_context(|| "Failed to connect with socket")?;
    let msg: Message = args.cmd.clone().into();
    stream
        .write_all(&msg.encode()?)
        .with_context(|| "Failed to write message")?;
    stream.shutdown(Shutdown::Write).unwrap();

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .with_context(|| "Failed to read response")?;
    let response = Response::decode(&buf).with_context(|| "")?;
    let mut stdout = io::stdout().lock();
    match response {
        Response::Ok => {}
        Response::Duration(d) => write!(stdout, "{}\n", format_duration(d))?,
        Response::Error(e) => match e {
            movebeam::ResponseError::NotFound => write!(stdout, "ERROR: Timer not found!\n")?,
        },
        Response::List(list) => {
            for (name, info) in list {
                write!(
                    stdout,
                    "{}\t{}/{}\n",
                    name,
                    format_duration(info.elapsed),
                    format_duration(info.interval)
                )?;
            }
        }
        Response::Timer(info) => {
            if let Command::Bar {
                name: _,
                size,
                fill,
                empty,
                left,
                right,
            } = args.cmd
            {
                let percentage = info.elapsed.as_secs_f64() / info.interval.as_secs_f64();
                let fill_count = (size as f64 * percentage).round() as usize;
                let bar_str = fill.repeat(fill_count) + &empty.repeat(size - fill_count);
                write!(stdout, "{}{}{}\n", left, bar_str, right)?;
            } else {
                write!(
                    stdout,
                    "{}/{}\n",
                    format_duration(info.elapsed),
                    format_duration(info.interval)
                )?;
            }
        }
    }
    Ok(())
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let m = secs / 60;
    let s = secs % 60;
    format!("{m:02}:{s:02}")
}
