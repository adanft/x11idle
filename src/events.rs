//! # System Events

use std::process::Command;

use crate::config::GeneralConfig;
use crate::error::Error;
use crate::output;

#[derive(Debug, Clone)]
pub enum Event {
    PhysicalInput,
    Sleep {
        going_to_sleep: bool,
    },
    Lock,
    Unlock,
    ScreenSaverInhibit {
        path: &'static str,
        application: String,
        reason: String,
        cookie: u32,
    },
    ScreenSaverUnInhibit {
        path: &'static str,
        cookie: u32,
        details: Option<(String, String)>,
    },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::PhysicalInput => "physical_input",
            Event::Sleep {
                going_to_sleep: true,
            } => "before_sleep",
            Event::Sleep {
                going_to_sleep: false,
            } => "after_sleep",
            Event::Lock => "lock",
            Event::Unlock => "unlock",
            Event::ScreenSaverInhibit { .. } => "screensaver_inhibit",
            Event::ScreenSaverUnInhibit { .. } => "screensaver_uninhibit",
        }
    }

    pub fn execute(&self, config: &GeneralConfig) {
        let cmd = match self {
            Event::PhysicalInput => &None,
            Event::Sleep {
                going_to_sleep: true,
            } => &config.before_sleep_cmd,
            Event::Sleep {
                going_to_sleep: false,
            } => &config.after_sleep_cmd,
            Event::Lock => &config.lock_cmd,
            Event::Unlock => &config.unlock_cmd,
            Event::ScreenSaverInhibit { .. } => &None,
            Event::ScreenSaverUnInhibit { .. } => &None,
        };

        if let Some(cmd_str) = cmd {
            run_command(self.name(), cmd_str);
        }
    }

    pub fn describe(&self) -> Option<String> {
        match self {
            Event::ScreenSaverInhibit {
                path,
                application,
                reason,
                cookie,
            } => Some(format!(
                "ScreenSaver Inhibit: path={} app={:?} reason={:?} cookie={}",
                path, application, reason, cookie,
            )),
            Event::ScreenSaverUnInhibit {
                path,
                cookie,
                details,
            } => match details {
                Some((application, reason)) => Some(format!(
                    "ScreenSaver UnInhibit: path={} cookie={} app={:?} reason={:?}",
                    path, cookie, application, reason,
                )),
                None => Some(format!(
                    "ScreenSaver UnInhibit: path={} cookie={} app=<unknown> reason=<unknown>",
                    path, cookie,
                )),
            },
            _ => None,
        }
    }
}

pub fn run_command(name: &str, cmd_str: &str) {
    output::debug(format!("Executing {} command: {}", name, cmd_str));

    if let Err(err) = Command::new("sh").arg("-c").arg(cmd_str).spawn() {
        let err = Error::Command(format!(
            "failed to spawn {} command '{}': {}",
            name, cmd_str, err
        ));
        output::error(err);
    }
}

pub fn run_optional_command(name: &str, cmd_str: &str) {
    if cmd_str.trim().is_empty() {
        return;
    }

    run_command(name, cmd_str);
}
