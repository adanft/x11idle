//! # Configuration
//!
//! x11idle reads its configuration from a TOML file located at:
//! - `$XDG_CONFIG_HOME/x11idle/config.toml`
//! - `~/.config/x11idle/config.toml`
//!
//! # Example Configuration
//!
//! ```toml
//! [general]
//! lock_cmd = "hyprlock"
//! unlock_cmd = "notify-send 'Welcome back!'"
//! before_sleep_cmd = "systemctl suspend"
//! after_sleep_cmd = "notify-send 'System awake'"
//!
//! [[listener]]
//! timeout = 300
//! on-timeout = "notify-send 'Idle timeout reached'"
//! on-resume = "notify-send 'Welcome back'"
//! ```

use std::path::PathBuf;
use std::{env, fs};

use serde::Deserialize;

use crate::error::Error;

/// Name of the config directory in XDG_CONFIG_HOME or ~/.config
const X11IDLE_DIR: &str = "x11idle";
/// Name of the configuration file
const CONFIG_FILE: &str = "config.toml";

/// Main configuration container
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    /// General settings for system events
    #[serde(default)]
    pub general: GeneralConfig,
    /// List of idle listeners with timeout and commands
    #[serde(default)]
    pub listener: Vec<ListenerConfig>,
}

/// General configuration for system events
///
/// These commands are executed in response to system events like lock, unlock,
/// and sleep. All commands are executed via `sh -c`.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct GeneralConfig {
    /// Command to execute when the session is locked
    ///
    /// This is triggered by the `Lock` signal from systemd-logind.
    /// Example: "hyprlock", "loginctl lock-session", "swaylock"
    #[serde(default)]
    pub lock_cmd: Option<String>,

    /// Command to execute when the session is unlocked
    ///
    /// This is triggered by the `Unlock` signal from systemd-logind.
    /// Example: "notify-send 'Welcome back!'"
    #[serde(default)]
    pub unlock_cmd: Option<String>,

    /// Command to execute BEFORE the system goes to sleep
    ///
    /// This runs when `PrepareForSleep(true)` is received from logind.
    /// The command runs before the system actually suspends/hibernates.
    /// Example: "systemctl suspend", "notify-send 'Going to sleep...'"
    #[serde(default)]
    pub before_sleep_cmd: Option<String>,

    /// Command to execute AFTER the system wakes from sleep
    ///
    /// This runs when `PrepareForSleep(false)` is received from logind.
    /// The command runs after the system has resumed from suspend/hibernate.
    /// Example: "notify-send 'Good morning!'"
    #[serde(default)]
    pub after_sleep_cmd: Option<String>,
}

/// Configuration for a single idle listener
///
/// Each listener defines an idle timeout (in seconds) and commands to execute
/// when that timeout is reached or when the user returns.
#[derive(Debug, Deserialize, Clone)]
pub struct ListenerConfig {
    /// Idle timeout in seconds
    ///
    /// The time in seconds of inactivity before the `on-timeout` command is triggered.
    /// The timer accounts for screensaver inhibition - when an application inhibits
    /// the screensaver, the idle timer pauses.
    ///
    /// Example: 300 (5 minutes), 600 (10 minutes), 1800 (30 minutes)
    pub timeout: u64,

    /// Command to execute when the idle timeout is reached
    ///
    /// This command is executed via `sh -c` when the user has been idle for
    /// `timeout` seconds without any physical input.
    ///
    /// Example: "notify-send 'Idle timeout reached'", "loginctl lock-session"
    #[serde(rename = "on-timeout")]
    pub on_timeout: String,

    /// Command to execute when the user returns after a timeout
    ///
    /// This command is executed via `sh -c` when physical input is detected
    /// after the timeout was triggered. It runs once per idle cycle.
    ///
    /// Example: "notify-send 'Welcome back'"
    #[serde(rename = "on-resume")]
    pub on_resume: String,
}

impl Config {
    /// Load configuration from the standard config locations
    ///
    /// Searches for config.toml in:
    /// 1. `$XDG_CONFIG_HOME/x11idle/config.toml`
    /// 2. `~/.config/x11idle/config.toml`
    ///
    /// If no config file is found, returns a default empty configuration.
    pub fn load() -> Result<Self, Error> {
        let config_path = Self::find_config_path();

        if let Some(path) = config_path {
            let content = fs::read_to_string(&path).map_err(|err| {
                Error::Config(format!("failed to read {}: {}", path.display(), err))
            })?;
            let config: Config = toml::from_str(&content).map_err(|err| {
                Error::Config(format!("failed to parse {}: {}", path.display(), err))
            })?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Find the configuration file path
    ///
    /// Checks XDG_CONFIG_HOME first, then falls back to ~/.config
    fn find_config_path() -> Option<PathBuf> {
        if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config)
                .join(X11IDLE_DIR)
                .join(CONFIG_FILE);
            if path.exists() {
                return Some(path);
            }
        }

        if let Ok(home) = env::var("HOME") {
            let path = PathBuf::from(home)
                .join(".config")
                .join(X11IDLE_DIR)
                .join(CONFIG_FILE);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }
}
