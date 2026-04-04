//! # Output / Logging

use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flag - set via CLI
static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

/// Initialize debug mode from CLI
pub fn set_debug_mode(enabled: bool) {
    DEBUG_MODE.store(enabled, Ordering::Relaxed);
}

/// Check if debug mode is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_MODE.load(Ordering::Relaxed)
}

/// Print info message to stdout
///
/// Always printed regardless of debug mode.
pub fn info(message: impl Display) {
    println!("{message}");
}

/// Print error message to stderr
///
/// Always printed regardless of debug mode.
pub fn error(message: impl Display) {
    eprintln!("{message}");
}

/// Print debug message to stderr
///
/// Only printed when debug mode is enabled via --debug flag
/// or X11IDLE_DEBUG environment variable.
pub fn debug(message: impl Display) {
    if is_debug_enabled() {
        eprintln!("[DEBUG] {message}");
    }
}
