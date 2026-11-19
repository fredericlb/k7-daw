//! warm.rs — a signed-in `/auth/me` starts the account's file service, so the files are warm by the time the page asks.

use crate::AppState;

/// Warms this front's hub for `email`, without waiting for it.
pub fn files(state: &AppState, email: &str) {
    if let Some(f) = state.files.clone().filter(|f| f.owned()) {
        let email = email.to_string();
        tokio::spawn(async move { f.hub.warm(&email).await });
    }
}
