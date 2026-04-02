//! # Command Line Interface

use clap::Parser;

/// Command line arguments for x11idle
#[derive(Parser, Debug)]
#[command(
    name = "x11idle",
    version = env!("CARGO_PKG_VERSION"),
    about = "X11 idle daemon with D-Bus integration",
    disable_help_flag = true
)]
pub struct Args {
    /// Enable debug logging to stderr
    #[arg(
        short,
        long,
        env = "X11IDLE_DEBUG",
        help = "Enable verbose debug output"
    )]
    pub debug: bool,

    /// Show help information
    #[arg(short, long, action = clap::ArgAction::Help, help = "Show this help message")]
    help: Option<bool>,
}

/// Get parsed CLI arguments
pub fn parse() -> Args {
    Args::parse()
}
