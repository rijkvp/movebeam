use anyhow::{Context, Result};
use movebeam::{
    msg::{ActivityMessage, Encoding, Response},
    socket::Socket,
};
use parking_lot::Mutex;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::Arc,
    thread,
    time::Instant,
};
use tracing::error;
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(
                    format!("{}=INFO", movebeam::ACTIVITY_DAEMON_NAME)
                        .as_str()
                        .parse()
                        .unwrap(),
                )
                .from_env()
                .unwrap(),
        )
        .init();
    ActivityDaemon::start()?.run();

    Ok(())
}

struct ActivityDaemon {
    socket: Socket,
    last_input: Arc<Mutex<Instant>>,
}

impl ActivityDaemon {
    fn start() -> Result<Self> {
        let (event_tx, event_rx) = crossbeam_channel::bounded(128);
        let last_input = Arc::new(Mutex::new(Instant::now()));

        {
            let last_input = last_input.clone();
            thread::spawn(move || {
                movebeam::input_listener::start_listener(event_tx);
            });
            thread::spawn(move || loop {
                if let Ok(_) = event_rx.recv() {
                    println!("RECEIVE!");
                    *last_input.lock() = Instant::now();
                }
            });
        }

        let socket = Socket::create(movebeam::activity_daemon_path(), true)?;
        Ok(Self { socket, last_input })
    }

    fn run(&mut self) {
        if let Ok((stream, _)) = self.socket.listener.accept() {
            if let Err(e) = self.handle_connection(stream) {
                error!("Failed to handle connection: {e}");
            }
        }
    }

    fn handle_connection(&mut self, mut stream: UnixStream) -> Result<()> {
        let mut msg_bytes = Vec::new();
        stream
            .read_to_end(&mut msg_bytes)
            .with_context(|| "Failed to read message")?;
        let command = ActivityMessage::decode(&msg_bytes)?;
        let response = match command {
            ActivityMessage::Get => Response::Duration(self.last_input.lock().elapsed()),
        };
        stream
            .write_all(&response.encode()?)
            .with_context(|| "Failed to write message")?;
        Ok(())
    }
}
