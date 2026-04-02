//! # x11idle
//!
//! Idle daemon for Linux that listens for D-Bus events from systemd-logind.

mod app;
mod cli;
mod config;
mod error;
mod events;
mod idle;
mod inhibit;
mod listeners;
mod logind;
mod output;
mod screensaver;
mod x11;

use app::App;
use config::Config;

fn main() {
    // Parse CLI arguments first
    let args = cli::parse();

    // Initialize debug mode
    output::set_debug_mode(args.debug);

    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            output::error(format!("Failed to load config: {}", e));
            std::process::exit(1);
        }
    };

    // Create and run the application
    let mut app = App::new(config);

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            output::error(format!("Failed to create Tokio runtime: {}", e));
            std::process::exit(1);
        }
    };

    if let Err(e) = rt.block_on(app.run()) {
        output::error(format!("Fatal error: {}", e));
        std::process::exit(1);
    }
}