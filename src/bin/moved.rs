use anyhow::{Context, Result};
use movebeam::{
    config::{Config, Timer},
    msg::{Encoding, Message, Response, ResponseError, TimerInfo},
    socket::Socket,
};
use parking_lot::Mutex;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, trace};
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

const HEARTBEAT: Duration = Duration::from_secs(1);

use clap::{command, Parser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path of configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive("movebeam=INFO".parse().unwrap())
                .from_env()
                .unwrap(),
        )
        .init();
    let args = Args::parse();

    Daemon::start(args)?.run()
}

struct State {
    startup: Instant,
    last_input: Instant,
    timers: Vec<(Timer, Instant)>,
}

impl State {
    fn new(config: Config) -> Self {
        let timers: Vec<(Timer, Instant)> = config
            .timers
            .into_iter()
            .map(|t| (t, Instant::now()))
            .collect();
        Self {
            startup: Instant::now(),
            last_input: Instant::now(),
            timers,
        }
    }
}

struct Daemon {
    state: Arc<Mutex<State>>,
    shutdown: Arc<AtomicBool>,
}

impl Daemon {
    fn start(args: Args) -> Result<Self> {
        let shutdown = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, shutdown.clone())?;

        let config_path = args.config.unwrap_or(movebeam::config_path()?);
        let config = Config::load_or_default(&config_path)?;
        let state = Arc::new(Mutex::new(State::new(config)));

        let socket = Socket::create(movebeam::daemon_socket_path(), false)?;
        Self::start_socket(shutdown.clone(), socket, state.clone());

        Ok(Self {
            shutdown,
            state,
        })
    }

    fn run(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            self.update();
            thread::sleep(HEARTBEAT);
        }
        Ok(())
    }

    fn start_socket(shutdown: Arc<AtomicBool>, socket: Socket, state: Arc<Mutex<State>>) {
        while !shutdown.load(Ordering::Relaxed) {
            if let Ok((stream, _)) = socket.listener.accept() {
                if let Err(e) = Self::handle_connection(state.clone(), stream) {
                    error!("Failed to handle connection: {e}");
                }
            }
        }
    }

    fn update(&mut self) {
        let mut state = self.state.lock();
        let input_elapsed = state.last_input.elapsed();
        for (timer, mut clock) in state.timers.iter_mut() {
            trace!("[UPDATE] {}: {:?}", timer.name, clock);
            if timer.duration.is_some() && Some(input_elapsed) > timer.duration {
                debug!("Reset {} due to inactivity", timer.name);
                clock = Instant::now();
            }
            if clock.elapsed() > timer.interval {
                info!("Timer {} went off", timer.name);
                if timer.notify {
                    movebeam::send_notification(
                        format!("Timer {} went off", timer.name),
                        "Time to take a break!".to_string(),
                    )
                }
            }
        }
    }

    fn handle_connection(state: Arc<Mutex<State>>, mut stream: UnixStream) -> Result<()> {
        let mut msg_bytes = Vec::new();
        stream
            .read_to_end(&mut msg_bytes)
            .with_context(|| "Failed to read message")?;
        let command = Message::decode(&msg_bytes)?;

        let mut state = state.lock();
        let response = match command {
            Message::List => Response::List(
                state
                    .timers
                    .iter()
                    .map(|(t, i)| {
                        (
                            t.name.clone(),
                            TimerInfo {
                                elapsed: i.elapsed(),
                                interval: t.interval,
                            },
                        )
                    })
                    .collect(),
            ),
            Message::Get(name) => state
                .timers
                .iter()
                .find(|(t, _)| t.name == name)
                .map(|(t, i)| {
                    Response::Timer(TimerInfo {
                        elapsed: i.elapsed(),
                        interval: t.interval,
                    })
                })
                .unwrap_or(Response::Error(ResponseError::NotFound)),
            Message::Reset(name) => {
                if let Some(mut timer) = state.timers.iter_mut().find(|(t, _)| t.name == name) {
                    timer.1 = Instant::now();
                    Response::Ok
                } else {
                    Response::Error(ResponseError::NotFound)
                }
            }
            Message::ResetAll => {
                for timer in state.timers.iter_mut() {
                    timer.1 = Instant::now();
                }
                Response::Ok
            }
            Message::Running => Response::Duration(state.startup.elapsed()),
        };

        let msg = response.encode()?;
        stream
            .write_all(&msg)
            .with_context(|| "Failed to write message")?;
        Ok(())
    }
}
