use anyhow::{Context, Result};
use clap::Parser;
use movebeam::{
    cli::{Cli, CliCommand},
    msg::{Encoding, Message, Response, ResponseError},
};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    time::Duration,
};

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut stream = UnixStream::connect(movebeam::daemon_socket_path())
        .with_context(|| "Failed to connect with socket")?;
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
            ResponseError::NotFound => write!(stdout, "ERROR: Timer not found!\n")?,
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
            if let CliCommand::Bar {
                name: _,
                size,
                fill,
                empty,
                left,
                right,
            } = args.cmd
            {
                let percentage =
                    (info.elapsed.as_secs_f64() / info.interval.as_secs_f64()).min(1.0);
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
