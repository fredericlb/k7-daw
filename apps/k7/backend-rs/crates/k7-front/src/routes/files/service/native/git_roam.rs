//! git_roam.rs — `GET /git/roam`: the workspaces the person added on any device, from the host's list store, and which
//! of them are here. What a reader gets wrong: the list holds names and remotes, never files, and a sweep that brings it up
//! to this device runs on every read and after anything that adds, connects or removes a workspace.

use std::path::Path;
use std::sync::Mutex;

use axum::response::Response;
use k7_file_service::workspaces::EXAMPLES;
use k7_file_service::workspaces::names::folder_names;
use k7_git::Repo;
use k7_roam::mine::{self, Local};
use k7_roam::store::{self, ListStore};
use serde_json::json;

use super::git::Ctx;
use crate::routes::files::json_answer;

/// One sweep at a time: it reads the device's file, then writes it back.
static SWEEPING: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The workspaces of this device with the remote each points at; Examples is every device's own and is not listed.
fn here(workspaces: &str) -> Vec<Local> {
    folder_names(workspaces)
        .into_iter()
        .filter(|n| n != EXAMPLES)
        .map(|name| {
            let root = Path::new(workspaces).join(&name);
            let remote = root
                .join(".git")
                .exists()
                .then(|| Repo::open(&root, &root.join(".git")).ok())
                .flatten()
                .and_then(|r| r.origin_config())
                .map(|o| o.url);
            Local { name, remote }
        })
        .collect()
}

/// Brings the store up to this device's workspaces; returns the device's name-to-id map.
pub fn sweep_in(
    store: &dyn ListStore,
    workspaces: &str,
    account_home: &str,
) -> std::collections::BTreeMap<String, String> {
    let _one = SWEEPING.lock().unwrap_or_else(|p| p.into_inner());
    let dir = Path::new(account_home);
    let mut mine = mine::load(dir);
    if mine::sweep(store, &mut mine, &here(workspaces), now()) {
        let _ = mine::save(dir, &mine);
    }
    mine
}

/// The sweep on the installed store, on a blocking thread, for a caller that does not wait for it.
pub fn sweep_later(workspaces: String, account_home: String) {
    let store = store::current();
    if !store.available() {
        return;
    }
    tokio::task::spawn_blocking(move || {
        sweep_in(store.as_ref(), &workspaces, &account_home);
    });
}

pub async fn list(ctx: &Ctx) -> Response {
    let store = store::current();
    if !store.available() {
        return json_answer(
            200,
            &json!({ "available": false, "entries": [], "here": [] }),
        );
    }
    let (workspaces, home) = (ctx.workspaces.clone(), ctx.account_home.clone());
    let answered = tokio::task::spawn_blocking(move || {
        let mine = sweep_in(store.as_ref(), &workspaces, &home);
        let entries = k7_roam::list::live(store.as_ref());
        json!({ "available": true, "entries": entries, "here": mine.values().collect::<Vec<_>>() })
    })
    .await;
    match answered {
        Ok(v) => json_answer(200, &v),
        Err(_) => json_answer(
            500,
            &json!({ "error": "the list worker stopped", "code": "io" }),
        ),
    }
}
