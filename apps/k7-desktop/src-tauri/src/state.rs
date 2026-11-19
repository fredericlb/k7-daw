// state.rs — the little the app knows, behind Arc<Mutex<>>: where its backend stands and the port its page is on.

use std::sync::{Arc, Mutex};

/// Where the backend process stands, which the app knows first-hand.
#[derive(Clone, PartialEq)]
pub enum Backend {
    Starting,
    /// Answering: the page can be opened.
    Ready,
    /// Gone, with why; a restart may be coming.
    Down(String),
}

pub struct AppState {
    pub backend: Backend,
    pub port: u16,
}

pub type SharedState = Arc<Mutex<AppState>>;

/// The words the splash shows while the page is not up.
pub fn line(b: &Backend) -> String {
    match b {
        Backend::Starting => "Starting K7 DAW…".into(),
        Backend::Ready => String::new(),
        Backend::Down(why) => format!("⚠ {why}"),
    }
}
