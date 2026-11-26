//! polls.rs — the five polls the part runs while the engine is up: interfaces (and the switch the ranking
//! asks for), the catalogue, the plugin instances, the sessions and the pairing lists. Each is one task that
//! pauses its own pace between runs and starts at once when kicked; a result is kept only if the engine it
//! came from is still the one running (`Shared::update`). A poll that gets no answer changes nothing.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::view::{edit_plugins_sig, plugins_sig, sessions_sig};
use super::{Shared, poller};
use crate::audio_policy;
use crate::protocol::AudioDevice;

pub fn spawn_all(shared: &Arc<Shared>) {
    poller(
        shared,
        |k| &k.devices,
        |s| async move {
            poll_devices(&s).await;
            s.paces.devices
        },
    );
    poller(
        shared,
        |k| &k.plugins,
        |s| async move {
            poll_plugins(&s).await;
            let known = s
                .view
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .plugin_count
                .is_some();
            // Ask hard until the scan lands, and while the engine says one runs, then slowly.
            if known && !s.scanning.load(Ordering::Relaxed) {
                s.paces.plugins
            } else {
                s.paces.plugins_fast
            }
        },
    );
    poller(
        shared,
        |k| &k.edit_plugins,
        |s| async move {
            poll_edit_plugins(&s).await;
            s.paces.edit_plugins
        },
    );
    poller(
        shared,
        |k| &k.sessions,
        |s| async move {
            poll_sessions(&s).await;
            s.paces.sessions
        },
    );
    poller(
        shared,
        |k| &k.pairing,
        |s| async move {
            poll_pairing(&s).await;
            s.paces.pairing
        },
    );
}

/// Every poll at once, as the engine comes up.
pub fn kick_all(s: &Shared) {
    s.kicks.devices.notify_one();
    s.kicks.plugins.notify_one();
    s.kicks.edit_plugins.notify_one();
    s.kicks.sessions.notify_one();
    s.kicks.pairing.notify_one();
}

/// The type the engine knows `name` by, which a switch must name when two types share a device name.
pub fn device_type_of(devices: &[AudioDevice], name: &str) -> String {
    devices
        .iter()
        .find(|d| d.name == name)
        .map(|d| d.device_type.clone())
        .unwrap_or_default()
}

/// The interfaces as shown, with the menu signature the ranking now in force makes of them.
pub fn show_devices(s: &Shared, generation: u64, devices: Vec<AudioDevice>) {
    let priority = s.cx.settings.borrow().audio_priority.clone();
    s.update(generation, |v| {
        v.menu_sig = audio_policy::menu_signature(&devices, &priority);
        v.devices = devices;
        true
    });
}

pub async fn poll_devices(s: &Shared) {
    if !s.up() {
        return;
    }
    let generation = s.client.generation();
    let Some(devices) = s.client.list_audio_devices().await else {
        return;
    };
    let priority = s.cx.settings.borrow().audio_priority.clone();
    let want = audio_policy::desired_switch(&devices, &priority).map(str::to_string);
    let set = audio_policy::signature(&devices);
    let attempt = want.map(|w| (w, set));
    let blocked = {
        let mut last = s.last_attempt.lock().unwrap_or_else(|e| e.into_inner());
        match &attempt {
            None => {
                *last = None;
                true
            }
            Some(a) if last.as_ref() == Some(a) => true,
            Some(a) => {
                *last = Some(a.clone());
                false
            }
        }
    };
    let mut shown = devices;
    if let (false, Some((name, _))) = (blocked, &attempt) {
        let kind = device_type_of(&shown, name);
        if let Some(updated) = s.client.set_audio_device(name, &kind).await {
            shown = updated;
        }
    }
    show_devices(s, generation, shown);
}

pub async fn poll_plugins(s: &Shared) {
    if !s.up() {
        return;
    }
    let generation = s.client.generation();
    let Some(catalogue) = s.client.list_plugins().await else {
        return;
    };
    s.scanning.store(catalogue.scanning, Ordering::Relaxed);
    let sig = plugins_sig(&catalogue.plugins);
    s.update(generation, |v| {
        // A rescan that found the same catalogue must still land: a null count is the spinner.
        if sig == v.plugins_sig && v.plugin_count.is_some() {
            return false;
        }
        v.plugin_count = Some(catalogue.plugins.len() as u32);
        v.plugins = catalogue.plugins;
        v.plugins_sig = sig;
        true
    });
}

async fn poll_edit_plugins(s: &Shared) {
    if !s.up() {
        return;
    }
    let generation = s.client.generation();
    let Some(plugins) = s.client.list_edit_plugins().await else {
        return;
    };
    let sig = edit_plugins_sig(&plugins);
    s.update(generation, |v| {
        if sig == v.edit_plugins_sig {
            return false;
        }
        v.edit_plugins = plugins;
        v.edit_plugins_sig = sig;
        true
    });
}

pub async fn poll_sessions(s: &Shared) {
    if !s.up() {
        return;
    }
    let generation = s.client.generation();
    let Some(sessions) = s.client.list_sessions().await else {
        return;
    };
    let sig = sessions_sig(&sessions);
    s.update(generation, |v| {
        if sig == v.sessions_sig {
            return false;
        }
        v.sessions = sessions;
        v.sessions_sig = sig;
        true
    });
}

/// Pending requests and paired devices together: both are short, and a request must reach the musician fast.
pub async fn poll_pairing(s: &Shared) {
    if !s.up() || s.pairing_busy.swap(true, Ordering::AcqRel) {
        return;
    }
    let generation = s.client.generation();
    let requests = s.client.list_pair_requests().await;
    let devices = s.client.list_paired_devices().await;
    s.pairing_busy.store(false, Ordering::Release);
    s.update(generation, |v| {
        let mut changed = false;
        if let Some(r) = requests.filter(|r| *r != v.pair_requests) {
            v.pair_requests = r;
            changed = true;
        }
        if let Some(d) = devices.filter(|d| *d != v.paired_devices) {
            v.paired_devices = d;
            changed = true;
        }
        changed
    });
}

/// Longest a command waits for the settings change it asked of the core to be seen.
pub const SETTINGS_SEEN: Duration = Duration::from_secs(1);
