//! commands.rs — the app's commands that act on the engine: the interface ranking, the LAN switch, the
//! rescan, a native plugin window, a dropped session, the pairing verbs. They run one after the other on a
//! task of their own, so two rankings reach the settings in the order they were sent; every one answers.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::timeout;

use super::Shared;
use super::polls::{SETTINGS_SEEN, device_type_of, poll_pairing, poll_sessions, show_devices};
use crate::audio_policy;
use crate::log;
use crate::protocol::{Command, UNIT_SEP, split_units};

pub const NAMES: &[&str] = &[
    "audio.priority",
    "lan.open",
    "plugins.rescan",
    "plugin.showUI",
    "session.drop",
    "pair.approve",
    "pair.deny",
    "pair.revoke",
];

pub fn spawn_worker(shared: &Arc<Shared>, mut commands: mpsc::UnboundedReceiver<Command>) {
    let s = shared.clone();
    tokio::spawn(async move {
        while let Some(c) = commands.recv().await {
            run(&s, c).await;
        }
    });
}

async fn run(s: &Shared, c: Command) {
    let ok = match c.cmd.as_str() {
        "audio.priority" => audio_priority(s, &c.arg).await,
        "lan.open" => {
            // `1` or `0`; remembered first, then told to the engine, which drops the LAN devices it let in when it closes.
            let open = c.arg == "1";
            s.cx.update_settings(move |x| x.lan_open = open);
            s.client.set_lan_open(open).await;
            true
        }
        "plugins.rescan" => {
            rescan(s).await;
            true
        }
        "plugin.showUI" => {
            // `<session>\x1f<id>`, an id alone naming one row PER SESSION.
            match c.arg.split_once(UNIT_SEP) {
                Some((session, id)) => s.client.show_native_plugin_ui(id, Some(session)).await,
                None => s.client.show_native_plugin_ui(&c.arg, None).await,
            };
            true
        }
        "session.drop" => {
            s.client.drop_session(&c.arg).await;
            // Polled at once: a row still listed for two seconds reads as a click that missed.
            poll_sessions(s).await;
            true
        }
        "pair.approve" | "pair.deny" | "pair.revoke" => {
            let verb = match c.cmd.as_str() {
                "pair.approve" => "pair-approve",
                "pair.deny" => "pair-deny",
                _ => "unpair-device",
            };
            s.client.pair_verb(verb, &c.arg).await;
            // Polled at once, like a dropped session: the row must leave the window with the click.
            poll_pairing(s).await;
            true
        }
        other => {
            s.cx.reply(c.id, false, &format!("{other} is not a supervisor command"));
            return;
        }
    };
    s.cx.reply(c.id, ok, "");
}

/// The WHOLE ranking in one command: a reorder is one act. Remembered first, then switched: a refused
/// device must not cost the user the order they just set.
async fn audio_priority(s: &Shared, arg: &str) -> bool {
    let mut ranked: Vec<String> = Vec::new();
    for n in split_units(arg) {
        if !ranked.contains(&n) {
            ranked.push(n);
        }
    }
    *s.last_attempt.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let want = ranked.clone();
    s.cx.update_settings(move |x| x.audio_priority = want);
    // The core saves and publishes it; what follows reads the ranking from the settings it now holds.
    let mut seen = s.cx.settings.clone();
    let _ = timeout(SETTINGS_SEEN, seen.wait_for(|x| x.audio_priority == ranked)).await;
    let generation = s.client.generation();
    let devices = s
        .view
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .devices
        .clone();
    show_devices(s, generation, devices.clone());
    let Some(name) = audio_policy::desired_switch(&devices, &ranked) else {
        return true;
    };
    let kind = device_type_of(&devices, name);
    match s.client.set_audio_device(name, &kind).await {
        Some(updated) => {
            show_devices(s, generation, updated);
            true
        }
        None => false,
    }
}

async fn rescan(s: &Shared) {
    // The spinner at once: the scan runs inside the engine and its count comes back through the poll.
    let generation = s.client.generation();
    s.update(generation, |v| {
        v.plugin_count = None;
        true
    });
    if let Err(e) = std::fs::remove_file(s.cx.paths.exts_cache())
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::host("could not delete exts.json — the rescan may return stale results");
    }
    s.client.rescan_plugins().await;
    s.kicks.plugins.notify_one();
}
