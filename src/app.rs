//! # Main Application
//!
//! Orchestrates all components: X11 idle clock, D-Bus listeners, event loop.
//! Uses a clear section structure to organize responsibilities.

use std::collections::HashSet;

use tokio::sync::mpsc;
use tokio::signal;
use tokio::task::JoinSet;
use tokio::time::{self, Duration, Instant};
use zbus::Connection;

use crate::config::Config;
use crate::error::Error;
use crate::events::Event;
use crate::idle::IdleScheduler;
use crate::inhibit::SleepInhibit;
use crate::listeners;
use crate::output;
use crate::screensaver;
use crate::x11::{IdleClock, PhysicalInputMonitor};

// =============================================================================
// Constants
// =============================================================================

const EVENT_CHANNEL_CAPACITY: usize = 32;
const IMMEDIATE_WAKE_DELAY_MS: u64 = 1;
const NO_IDLE_CLOCK_DELAY_SECS: u64 = 60;
const MAINTENANCE_DELAY_SECS: u64 = 5;

// =============================================================================
// App State
// =============================================================================

pub struct App {
    // Configuration
    config: Config,
    // System integration
    inhibit: Option<SleepInhibit>,
    // X11 integration
    idle_clock: Option<IdleClock>,
    physical_input_monitor: Option<PhysicalInputMonitor>,
    // Idle tracking
    idle_scheduler: IdleScheduler,
    screensaver_inhibitors: HashSet<u32>,
    // Event communication
    event_tx: Option<mpsc::Sender<Event>>,
}

impl App {
    // --------------------------------------------------------------------------
    // Construction
    // --------------------------------------------------------------------------

    pub fn new(config: Config) -> Self {
        let idle_scheduler = IdleScheduler::new(config.listener.clone());

        Self {
            config,
            inhibit: None,
            idle_clock: None,
            physical_input_monitor: None,
            idle_scheduler,
            screensaver_inhibitors: HashSet::new(),
            event_tx: None,
        }
    }

    // --------------------------------------------------------------------------
    // Startup
    // --------------------------------------------------------------------------

    pub async fn run(&mut self) -> Result<(), Error> {
        output::info("Starting x11idle...");
        output::info(format!("Configuration: {:?}", self.config.general));
        output::info(format!("Configured listeners: {}", self.config.listener.len()));

        // Initialize X11 idle clock if we have listeners
        if !self.idle_scheduler.is_empty() {
            self.idle_clock = Some(IdleClock::new()?);
            output::info("Connected to X11 idle clock");
        }

        // Connect to D-Bus
        let conn = Connection::system().await?;
        output::info("Connected to system D-Bus");

        // Get session path for lock/unlock signals
        let session_path = listeners::get_session_path(&conn).await?;
        output::info(format!("Session: {}", session_path.as_str()));

        // Inhibit sleep while running
        self.inhibit_sleep(&conn).await?;

        // Create event channel and spawn listeners
        let (tx, rx) = mpsc::channel::<Event>(EVENT_CHANNEL_CAPACITY);
        self.event_tx = Some(tx.clone());

        let mut listeners = JoinSet::new();
        let sleep_conn = conn.clone();
        let lock_conn = conn.clone();
        let unlock_conn = conn.clone();
        let screensaver_tx = tx.clone();
        let sleep_tx = tx.clone();
        let lock_tx = tx.clone();
        let lock_session_path = session_path.clone();

        listeners.spawn(async move { listeners::listen_sleep(sleep_conn, sleep_tx).await.map_err(Error::from) });
        listeners.spawn(async move { listeners::listen_lock(lock_conn, lock_session_path, lock_tx).await.map_err(Error::from) });
        listeners.spawn(async move { listeners::listen_unlock(unlock_conn, session_path, tx).await.map_err(Error::from) });
        listeners.spawn(async move { screensaver::serve(screensaver_tx).await.map_err(Error::from) });

        output::info("Listeners started, entering event loop...");

        self.event_loop(&conn, rx, &mut listeners).await
    }

    // --------------------------------------------------------------------------
    // Event Loop
    // --------------------------------------------------------------------------

    async fn event_loop(
        &mut self,
        conn: &Connection,
        mut rx: mpsc::Receiver<Event>,
        listeners: &mut JoinSet<Result<(), Error>>,
    ) -> Result<(), Error> {
        let idle_sleep = time::sleep(self.initial_idle_delay());
        tokio::pin!(idle_sleep);

        loop {
            tokio::select! {
                // Handle incoming events from D-Bus or X11
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            self.handle_event(conn, event).await?;
                            idle_sleep.as_mut().reset(Instant::now() + self.immediate_idle_delay());
                        }
                        None => {
                            output::error("Event channel closed, stopping x11idle...");
                            self.release_inhibit();
                            break;
                        }
                    }
                }
                // Graceful shutdown on Ctrl+C
                _ = signal::ctrl_c() => {
                    output::info("Shutting down gracefully...");
                    self.release_inhibit();
                    break;
                }
                // Monitor listener tasks for crashes
                listener = listeners.join_next(), if !listeners.is_empty() => {
                    match listener {
                        Some(Ok(Ok(()))) => {
                            self.release_inhibit();
                            return Err(Error::Channel("A listener stopped unexpectedly".into()));
                        }
                        Some(Ok(Err(err))) => {
                            self.release_inhibit();
                            return Err(err);
                        }
                        Some(Err(err)) => {
                            self.release_inhibit();
                            return Err(Error::Channel(format!("A listener task panicked: {}", err)));
                        }
                        None => {}
                    }
                }
                // Periodic X11 idle polling
                _ = &mut idle_sleep, if self.idle_clock.is_some() => {
                    let next_delay = self.poll_x11()?;
                    idle_sleep.as_mut().reset(Instant::now() + next_delay);
                }
            }
        }
        
        output::info("Event loop finished");
        Ok(())
    }

    // --------------------------------------------------------------------------
    // Event Handling
    // --------------------------------------------------------------------------

    async fn handle_event(&mut self, conn: &Connection, event: Event) -> Result<(), Error> {
        match &event {
            Event::Sleep { going_to_sleep: true } => {
                output::debug(format!("Event received: {}", event.name()));
                event.execute(&self.config.general);
                self.release_inhibit();
            }
            Event::Sleep { going_to_sleep: false } => {
                output::debug(format!("Event received: {}", event.name()));
                event.execute(&self.config.general);
                if let Err(e) = self.inhibit_sleep(conn).await {
                    output::error(format!("Failed to re-inhibit sleep after wake: {}", e));
                }
            }
            Event::PhysicalInput => {
                output::debug("Event received: physical_input");
                self.handle_physical_activity();
            }
            Event::Lock | Event::Unlock => {
                output::debug(format!("Event received: {}", event.name()));
                event.execute(&self.config.general);
            }
            Event::ScreenSaverInhibit { .. } | Event::ScreenSaverUnInhibit { .. } => {
                if let Some(summary) = event.describe() {
                    output::debug(summary);
                }

                match &event {
                    Event::ScreenSaverInhibit { cookie, .. } => {
                        self.screensaver_inhibitors.insert(*cookie);
                    }
                    Event::ScreenSaverUnInhibit { cookie, .. } => {
                        self.screensaver_inhibitors.remove(cookie);
                    }
                    _ => {}
                }
            }
        }

        self.sync_physical_input_monitor()?;
        Ok(())
    }

    // --------------------------------------------------------------------------
    // Idle Management
    // --------------------------------------------------------------------------

    fn poll_x11(&mut self) -> Result<Duration, Error> {
        let Some(idle_clock) = &self.idle_clock else {
            return Ok(self.idle_maintenance_delay());
        };

        match idle_clock.idle_ms() {
            Ok(idle_ms) => {
                let inhibit_active = !self.screensaver_inhibitors.is_empty();
                self.idle_scheduler.update(idle_ms, inhibit_active);
                self.sync_physical_input_monitor()?;
                Ok(self.compute_next_idle_delay(idle_ms, inhibit_active))
            }
            Err(err) => {
                output::error(err.to_string());
                Ok(self.idle_maintenance_delay())
            }
        }
    }

    fn initial_idle_delay(&self) -> Duration {
        if self.idle_clock.is_some() {
            self.immediate_idle_delay()
        } else {
            Duration::from_secs(NO_IDLE_CLOCK_DELAY_SECS)
        }
    }

    fn compute_next_idle_delay(&self, idle_ms: u64, inhibit_active: bool) -> Duration {
        self.idle_scheduler.next_wake_delay(idle_ms, inhibit_active)
    }

    fn idle_maintenance_delay(&self) -> Duration {
        Duration::from_secs(MAINTENANCE_DELAY_SECS)
    }

    fn immediate_idle_delay(&self) -> Duration {
        Duration::from_millis(IMMEDIATE_WAKE_DELAY_MS)
    }

    fn handle_physical_activity(&mut self) {
        self.idle_scheduler.handle_user_activity();
        self.physical_input_monitor = None;
    }

    // --------------------------------------------------------------------------
    // Physical Input Monitor
    // --------------------------------------------------------------------------

    /// Starts or stops the physical input monitor based on pending resumes.
    fn sync_physical_input_monitor(&mut self) -> Result<(), Error> {
        let should_listen = self.idle_scheduler.has_pending_resumes();

        match (should_listen, self.physical_input_monitor.is_some()) {
            (true, false) => {
                let tx = self
                    .event_tx
                    .as_ref()
                    .ok_or_else(|| Error::Channel("event sender unavailable".into()))?
                    .clone();
                self.physical_input_monitor = Some(PhysicalInputMonitor::new(tx)?);
                output::debug("Enabled X11 physical input monitoring for pending resumes");
            }
            (false, true) => {
                self.physical_input_monitor = None;
                output::debug("Disabled X11 physical input monitoring");
            }
            _ => {}
        }

        Ok(())
    }

    // --------------------------------------------------------------------------
    // Sleep Inhibition
    // --------------------------------------------------------------------------

    async fn inhibit_sleep(&mut self, conn: &Connection) -> Result<(), Error> {
        let inhibit = SleepInhibit::new(conn).await?;
        self.inhibit = Some(inhibit);
        Ok(())
    }

    fn release_inhibit(&mut self) {
        if let Some(inhibit) = &mut self.inhibit {
            inhibit.release();
        }
    }
}