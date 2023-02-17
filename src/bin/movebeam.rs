use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use movebeam::{
    config::{Config, Timer},
    input_listener::InputEvent,
    Command, Response, ResponseError, Serialization, TimerInfo,
};
use parking_lot::Mutex;
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tracing::{debug, error, info};
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};
use std::os::unix::fs::PermissionsExt;

const HEARTBEAT: Duration = Duration::from_secs(1);

fn main() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive("movebeam=INFO".parse().unwrap())
        .from_env()
        .unwrap();
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    Daemon::start()?.run()
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
    socket_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    state: Arc<Mutex<State>>,
    event_rx: Receiver<InputEvent>,
}

impl Daemon {
    fn start() -> Result<Self> {
        let shutdown = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, shutdown.clone())?;

        let config = Config::load_or_create(&movebeam::config_path()?)?;
        let state = Arc::new(Mutex::new(State::new(config)));

        let (event_tx, event_rx) = crossbeam_channel::bounded(128);
        Self::start_input_listener(event_tx);

        let socket_path = movebeam::socket_path();
        Self::start_socket(&socket_path, shutdown.clone(), state.clone())?;

        Ok(Self {
            socket_path,
            shutdown,
            state,
            event_rx,
        })
    }

    fn start_input_listener(event_tx: Sender<InputEvent>) {
        thread::spawn(move || {
            movebeam::input_listener::start_listener(event_tx);
        });
    }

    fn start_socket(
        socket_path: &Path,
        shutdown: Arc<AtomicBool>,
        state: Arc<Mutex<State>>,
    ) -> Result<JoinHandle<()>> {
        if socket_path.exists() {
            info!("Removing exsisting socket '{}'", socket_path.display());
            fs::remove_file(&socket_path).with_context(|| "Failed to remove existing socket")?;
        }
        if let Some(dir) = socket_path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create runtime directory {dir:?}"))?;
        }
        let socket = UnixListener::bind(&socket_path)
            .with_context(|| format!("Failed to bind socket at {socket_path:?}"))?;
        // Set permissions so that all users can write to the socket
        fs::set_permissions(socket_path, fs::Permissions::from_mode(0o722)).unwrap();
        info!("Running at '{}'", socket_path.display());
        Ok(thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                if let Ok((stream, _)) = socket.accept() {
                    if let Err(e) = Self::handle_connection(state.clone(), stream) {
                        error!("Failed to handle connection: {e}");
                    }
                }
            }
        }))
    }

    fn run(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            self.update();
            thread::sleep(HEARTBEAT);
        }
        Ok(())
    }

    fn update(&mut self) {
        let mut state = self.state.lock();
        if self.event_rx.try_iter().count() > 0 {
            state.last_input = Instant::now();
        }
        let input_elapsed = state.last_input.elapsed();
        for timer in state.timers.iter_mut() {
            if Some(input_elapsed) > timer.0.duration {
                debug!("Reset {} due to inactivity", timer.0.name);
                timer.1 = Instant::now();
            }
            if timer.1.elapsed() > timer.0.interval {
                info!("Timer {} went off", timer.0.name);
                if timer.0.notify {
                    movebeam::send_notification(
                        format!("Timer {} went off", timer.0.name),
                        "Time to take a break!".to_string(),
                    )
                }
            }
        }
    }

    fn handle_connection(state: Arc<Mutex<State>>, mut stream: UnixStream) -> Result<()> {
        // Receive command
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .with_context(|| "Failed to read message")?;
        let command = Command::decode(&buf)?;

        let mut state = state.lock();
        // Execute command
        let response = match command {
            Command::List => Response::List(
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
            Command::Get { name } => state
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
            Command::Reset { name } => {
                if let Some(mut timer) = state.timers.iter_mut().find(|(t, _)| t.name == name) {
                    timer.1 = Instant::now();
                    Response::Ok
                } else {
                    Response::Error(ResponseError::NotFound)
                }
            }
            Command::ResetAll => {
                for timer in state.timers.iter_mut() {
                    timer.1 = Instant::now();
                }
                Response::Ok
            }
            Command::Inactive => Response::Duration(state.last_input.elapsed()),
            Command::Running => Response::Duration(state.startup.elapsed()),
        };

        // Send response
        let msg = response.encode()?;
        stream
            .write_all(&msg)
            .with_context(|| "Failed to write message")?;
        Ok(())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        fs::remove_file(&self.socket_path).unwrap();
    }
}
