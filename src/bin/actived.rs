use anyhow::Result;
use movebeam::{msg::Encoding, socket::SocketServer};
use parking_lot::Mutex;
use std::{sync::Arc, thread, time::SystemTime};
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::builder().from_env().unwrap())
        .init();
    let (event_tx, event_rx) = crossbeam_channel::bounded(128);

    let last_input = Arc::new(Mutex::new(SystemTime::now()));
    {
        let last_input = last_input.clone();
        thread::spawn(move || {
            movebeam::input_listener::start_listener(event_tx);
        });
        thread::spawn(move || loop {
            if event_rx.recv().is_ok() {
                *last_input.lock() = SystemTime::now();
            }
        });
    }

    let mut socket = SocketServer::create(movebeam::activity_daemon_socket(), true)?;
    socket.serve(|_| Some(last_input.lock().encode().unwrap()))?;

    Ok(())
}
