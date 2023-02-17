use anyhow::{Context, Result};
use clap::Parser;
use movebeam::{Command, Response, Serialization};
use std::{
    io::{Read, Write},
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

fn main() -> Result<()> {
    let args = Args::parse();
    let socket_path = movebeam::socket_path()?;
    let mut stream =
        UnixStream::connect(socket_path).with_context(|| "Failed to connect with socket")?;
    let msg = args.cmd.encode()?;
    stream
        .write_all(&msg)
        .with_context(|| "Failed to write message")?;
    stream.shutdown(Shutdown::Write).unwrap();

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .with_context(|| "Failed to read response")?;
    let response = Response::decode(&buf).with_context(|| "")?;
    match response {
        Response::Ok => {}
        Response::Duration(d) => println!("{}", format_duration(d)),
        Response::Error(e) => match e {
            movebeam::ResponseError::NotFound => eprintln!("error: Couldn't find timer!"),
        },
        Response::List(list) => {
            for (name, dur) in list {
                println!("{name} {}", format_duration(dur));
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
