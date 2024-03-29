use anyhow::{Context, Result};
use clap::Parser;
use movebeam::{
    cli::{Cli, CliCommand},
    msg::{Encoding, Message, Response, ResponseError},
    socket::SocketClient,
};
use std::time::Duration;
use std::{io::Write, time::SystemTime};

fn main() -> Result<()> {
    let args = Cli::parse();

    let mut client = SocketClient::connect(movebeam::daemon_socket())?;
    let msg: Message = args.cmd.clone().into();
    let res_bytes = client.send(&msg.encode()?)?;
    let response = Response::decode(&res_bytes).with_context(|| "")?;
    let mut stdout = std::io::stdout().lock();
    match response {
        Response::Ok => {}
        Response::Duration(d) => writeln!(stdout, "{}", format_duration(d))?,
        Response::Error(e) => match e {
            ResponseError::NotFound => writeln!(stdout, "ERROR: Timer not found!")?,
        },
        Response::List(list) => {
            for (name, info) in list {
                writeln!(
                    stdout,
                    "{}\t{}/{}",
                    name,
                    format_duration(info.elapsed),
                    format_duration(info.interval)
                )?;
            }
        }
        Response::Timer(info) => {
            if let CliCommand::Bar {
                name: _,
                size,
                fill,
                empty,
                left,
                right,
                blink,
            } = args.cmd
            {
                let percentage =
                    (info.elapsed.as_secs_f64() / info.interval.as_secs_f64()).min(1.0);
                let bar_str = if percentage >= 1.0 && blink {
                    let unix_time = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if unix_time % 2 == 0 {
                        // Blink on
                        fill.repeat(size)
                    } else {
                        // Blink off
                        empty.repeat(size)
                    }
                } else {
                    let fill_count = (size as f64 * percentage).round() as usize;
                    fill.repeat(fill_count) + &empty.repeat(size - fill_count)
                };
                writeln!(stdout, "{}{}{}", left, bar_str, right)?;
            } else {
                writeln!(
                    stdout,
                    "{}/{}",
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
