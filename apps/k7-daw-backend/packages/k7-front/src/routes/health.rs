//! health.rs — `/api/health`: no session, no disk, only that the loop answers and which release answers.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;

pub async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "ok": true, "version": env!("K7_VERSION"), "build": state.settings.build }))
}
