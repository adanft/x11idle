use std::collections::HashMap;
use std::future::pending;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use zbus::{connection, interface, Result};

use crate::events::Event;
use crate::output;

pub const SERVICE: &str = "org.freedesktop.ScreenSaver";
pub const PRIMARY_PATH: &str = "/org/freedesktop/ScreenSaver";
pub const LEGACY_PATH: &str = "/ScreenSaver";

pub async fn serve(tx: mpsc::Sender<Event>) -> Result<()> {
    let state = Arc::new(Mutex::new(State {
        next_cookie: 1,
        active_cookies: HashMap::new(),
    }));

    let primary = ScreenSaverService::new(tx.clone(), PRIMARY_PATH, Arc::clone(&state));
    let legacy = ScreenSaverService::new(tx, LEGACY_PATH, Arc::clone(&state));

    let _conn = connection::Builder::session()?
        .name(SERVICE)?
        .serve_at(PRIMARY_PATH, primary)?
        .serve_at(LEGACY_PATH, legacy)?
        .build()
        .await?;

    output::debug("Serving org.freedesktop.ScreenSaver on /org/freedesktop/ScreenSaver and /ScreenSaver...");

    pending::<()>().await;
    Ok(())
}

struct ScreenSaverService {
    tx: mpsc::Sender<Event>,
    path: &'static str,
    state: Arc<Mutex<State>>,
}

struct State {
    next_cookie: u32,
    active_cookies: HashMap<u32, CookieDetails>,
}

#[derive(Clone)]
struct CookieDetails {
    application: String,
    reason: String,
}

impl ScreenSaverService {
    fn new(tx: mpsc::Sender<Event>, path: &'static str, state: Arc<Mutex<State>>) -> Self {
        Self { tx, path, state }
    }

    fn allocate_cookie(&self, application: &str, reason: &str) -> u32 {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let cookie = state.next_cookie;
        state.next_cookie = state.next_cookie.saturating_add(1);
        state.active_cookies.insert(
            cookie,
            CookieDetails {
                application: application.to_string(),
                reason: reason.to_string(),
            },
        );
        cookie
    }

    fn remove_cookie(&self, cookie: u32) -> Option<CookieDetails> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active_cookies.remove(&cookie)
    }
}

#[interface(name = "org.freedesktop.ScreenSaver")]
impl ScreenSaverService {
    async fn inhibit(&mut self, application_name: &str, reason_for_inhibit: &str) -> u32 {
        let cookie = self.allocate_cookie(application_name, reason_for_inhibit);

        let _ = self
            .tx
            .send(Event::ScreenSaverInhibit {
                path: self.path,
                application: application_name.to_string(),
                reason: reason_for_inhibit.to_string(),
                cookie,
            })
            .await;

        cookie
    }

    async fn un_inhibit(&mut self, cookie: u32) {
        let details = self.remove_cookie(cookie);

        let _ = self
            .tx
            .send(Event::ScreenSaverUnInhibit {
                path: self.path,
                cookie,
                details: details.map(|details| (details.application, details.reason)),
            })
            .await;
    }
}
