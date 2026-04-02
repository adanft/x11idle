//! # Error types for x11idle

#[derive(Debug)]
pub enum Error {
    Dbus(String),
    Config(String),
    Command(String),
    Inhibit(String),
    X11(String),
    Channel(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Dbus(msg) => write!(f, "D-Bus error: {}", msg),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::Command(msg) => write!(f, "Command error: {}", msg),
            Error::Inhibit(msg) => write!(f, "Inhibit error: {}", msg),
            Error::X11(msg) => write!(f, "X11 error: {}", msg),
            Error::Channel(msg) => write!(f, "Channel error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<zbus::Error> for Error {
    fn from(err: zbus::Error) -> Self {
        Error::Dbus(err.to_string())
    }
}

impl From<tokio::sync::mpsc::error::SendError<crate::events::Event>> for Error {
    fn from(err: tokio::sync::mpsc::error::SendError<crate::events::Event>) -> Self {
        Error::Channel(err.to_string())
    }
}
