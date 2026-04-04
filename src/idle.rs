use crate::config::ListenerConfig;
use crate::events;
use std::time::Duration;

const MILLIS_PER_SECOND: u64 = 1_000;
const MIN_WAKE_DELAY_MS: u64 = 50;
const IDLE_MAINTENANCE_DELAY_MS: u64 = 5_000;

pub struct IdleScheduler {
    listeners: Vec<ListenerState>,
    baseline_ms: u64,
    inhibit_active: bool,
}

struct ListenerState {
    config: ListenerConfig,
    timeout_ms: u64,
    fired: bool,
}

impl IdleScheduler {
    pub fn new(listeners: Vec<ListenerConfig>) -> Self {
        let mut listeners = listeners
            .into_iter()
            .map(|config| ListenerState {
                timeout_ms: config.timeout.saturating_mul(MILLIS_PER_SECOND),
                config,
                fired: false,
            })
            .collect::<Vec<_>>();

        listeners.sort_by_key(|listener| listener.timeout_ms);

        Self {
            listeners,
            baseline_ms: 0,
            inhibit_active: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }

    pub fn update(&mut self, idle_ms: u64, inhibit_active: bool) {
        if self.inhibit_active && !inhibit_active {
            self.baseline_ms = idle_ms;
        }

        self.inhibit_active = inhibit_active;

        if !self.inhibit_active {
            let effective_idle_ms = idle_ms.saturating_sub(self.baseline_ms);

            for listener in &mut self.listeners {
                if listener.fired || effective_idle_ms < listener.timeout_ms {
                    continue;
                }

                if let Some(cmd) = &listener.config.on_timeout {
                    events::run_command("listener timeout", cmd);
                }
                listener.fired = true;
            }
        }
    }

    pub fn has_pending_resumes(&self) -> bool {
        self.listeners.iter().any(|listener| listener.fired)
    }

    pub fn handle_user_activity(&mut self) {
        self.run_resumes();
        self.baseline_ms = 0;
    }

    pub fn next_wake_delay(&self, idle_ms: u64, inhibit_active: bool) -> Duration {
        if self.listeners.is_empty() {
            return Duration::from_millis(IDLE_MAINTENANCE_DELAY_MS);
        }

        if inhibit_active {
            return Duration::from_millis(IDLE_MAINTENANCE_DELAY_MS);
        }

        let effective_idle_ms = idle_ms.saturating_sub(self.baseline_ms);
        let next_timeout_ms = self
            .listeners
            .iter()
            .filter(|listener| !listener.fired)
            .map(|listener| listener.timeout_ms)
            .min();

        match next_timeout_ms {
            Some(timeout_ms) if timeout_ms > effective_idle_ms => Duration::from_millis(
                timeout_ms
                    .saturating_sub(effective_idle_ms)
                    .max(MIN_WAKE_DELAY_MS),
            ),
            Some(_) => Duration::from_millis(MIN_WAKE_DELAY_MS),
            None => Duration::from_millis(IDLE_MAINTENANCE_DELAY_MS),
        }
    }

    fn run_resumes(&mut self) {
        for listener in self.listeners.iter_mut().rev() {
            if !listener.fired {
                continue;
            }

            if let Some(cmd) = &listener.config.on_resume {
                events::run_command("listener resume", cmd);
            }
            listener.fired = false;
        }
    }
}
