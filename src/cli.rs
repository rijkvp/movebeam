use clap::{command, Parser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: CliCommand,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum CliCommand {
    /// List of information from all timers
    List,
    /// Get the information of a specific timer
    Get { name: String },
    /// Status bar
    Bar {
        name: String,
        #[clap(short, long, default_value_t = 16)]
        size: usize,
        #[clap(short, long, default_value_t = String::from("█"))]
        fill: String,
        #[clap(short, long, default_value_t = String::from("░"))]
        empty: String,
        #[clap(short, long, default_value_t = String::from("▕"))]
        left: String,
        #[clap(short, long, default_value_t = String::from("▏"))]
        right: String,
        #[clap(short, long)]
        blink: bool,
    },
    /// Reset a specific timer
    Reset { name: String },
    /// Reset all timers
    ResetAll,
}
