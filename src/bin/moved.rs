use anyhow::Result;
use clap::{command, Parser};
use movebeam::{
    config::{Config, TimerConfig},
    msg::{Encoding, Message, Response, ResponseError, TimerInfo},
    socket::{SocketClient, SocketServer},
};
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};
use tracing::{error, info, trace};
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

const HEARTBEAT: Duration = Duration::from_secs(1);

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
        .with(EnvFilter::builder().from_env().unwrap())
        .init();
    let args = Args::parse();

    Daemon::start(args)?.run()
}

struct TimerState {
    clock: Duration,
    went_off: bool,
    config: TimerConfig,
}

struct State {
    config: Config,
    activity_daemon_client: Option<SocketClient>,
    timers: Vec<TimerState>,
    last_update: Instant,
}

impl State {
    fn init(config: Config) -> Result<Self> {
        let timers: Vec<TimerState> = config
            .timers
            .iter()
            .map(|t| TimerState {
                clock: Duration::ZERO,
                went_off: false,
                config: t.clone(),
            })
            .collect();
        let activity_daemon_client = if config.activity.is_some() {
            Some(SocketClient::connect(movebeam::activity_daemon_socket())?)
        } else {
            None
        };
        Ok(Self {
            config,
            activity_daemon_client,
            timers,
            last_update: Instant::now(),
        })
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
        let state = Arc::new(Mutex::new(State::init(config)?));

        let socket = SocketServer::create(movebeam::daemon_socket(), false)?;
        Self::start_socket(socket, shutdown.clone(), state.clone());

        Ok(Self { shutdown, state })
    }

    fn run(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            {
                let mut state = self.state.lock();
                Self::update(&mut state)?;
            }
            thread::sleep(HEARTBEAT);
        }
        Ok(())
    }

    fn start_socket(mut socket: SocketServer, shutdown: Arc<AtomicBool>, state: Arc<Mutex<State>>) {
        thread::spawn(move || {
            socket
                .serve_until(shutdown, |msg| {
                    match Self::handle_connection(state.clone(), msg) {
                        Ok(msg) => Some(msg),
                        Err(e) => {
                            error!("Failed to handle connection: {e}");
                            None
                        }
                    }
                })
                .unwrap();
        });
    }

    fn update(state: &mut State) -> Result<()> {
        let input_elapsed = if let Some(client) = &mut state.activity_daemon_client {
            let resp = client.send(&[1])?;
            Some(SystemTime::decode(&resp)?.elapsed()?)
        } else {
            None
        };

        let mut reset = false;
        let delta = state.last_update.elapsed();

        let (inactivity_pause, inactivity_reset) = if let Some(activity) = &state.config.activity {
            (activity.inactivity_pause, activity.inactivity_reset)
        } else {
            (None, None)
        };

        // Reset when inactive
        // Also checks for the delta to be bigger which can happen when pc was in sleep
        if inactivity_reset.is_some()
            && (input_elapsed >= inactivity_reset || Some(delta) >= inactivity_reset)
        {
            reset = true;
        }

        for timer in state.timers.iter_mut() {
            trace!(
                "Update {}, clock: {:?}, interval: {:?}",
                timer.config.name,
                timer.clock,
                timer.config.interval
            );
            if timer.config.duration.is_some() && input_elapsed > timer.config.duration {
                // Rest if over break duration
                timer.clock = Duration::ZERO;
                reset = true;
            }

            if reset {
                info!("Reset timer {}", timer.config.name);
                timer.clock = Duration::ZERO;
                continue;
            }

            if input_elapsed <= inactivity_pause {
                // Only update clock if not paused
                timer.clock += delta;
            }

            if !timer.went_off && timer.clock > timer.config.interval {
                info!("Timer {} went off", timer.config.name);
                if timer.config.notify {
                    movebeam::send_notification(
                        format!("Timer {} went off", timer.config.name),
                        "Time to take a break!".to_string(),
                    )
                }
                timer.went_off = true;
            }
        }
        state.last_update = Instant::now();
        Ok(())
    }

    fn handle_connection(state: Arc<Mutex<State>>, msg: &[u8]) -> Result<Vec<u8>> {
        let command = Message::decode(msg)?;
        let mut state = state.lock();
        let response = match command {
            Message::List => Response::List(
                state
                    .timers
                    .iter()
                    .map(|t| {
                        (
                            t.config.name.clone(),
                            TimerInfo {
                                elapsed: t.clock,
                                interval: t.config.interval,
                            },
                        )
                    })
                    .collect(),
            ),
            Message::Get(name) => state
                .timers
                .iter()
                .find(|t| t.config.name == name)
                .map(|t| {
                    Response::Timer(TimerInfo {
                        elapsed: t.clock,
                        interval: t.config.interval,
                    })
                })
                .unwrap_or(Response::Error(ResponseError::NotFound)),
            Message::Reset(name) => {
                if let Some(timer) = state.timers.iter_mut().find(|t| t.config.name == name) {
                    timer.clock = Duration::ZERO;
                    Response::Ok
                } else {
                    Response::Error(ResponseError::NotFound)
                }
            }
            Message::ResetAll => {
                for timer in state.timers.iter_mut() {
                    timer.clock = Duration::ZERO;
                }
                Response::Ok
            }
        };
        response.encode()
    }
}
