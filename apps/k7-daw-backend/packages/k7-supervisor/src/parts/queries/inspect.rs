//! inspect.rs — what the part asks the engine about ITSELF: at every start, its `whoami` (an adopted engine
//! of another protocol or build, or one in safe mode, is replaced once; a safe one names its suspect), the
//! LAN setting told again, and the hang watch that kills an engine up but frozen. The rules:
//! `apps/k7-daw-engine/k7-supervisor/docs/crashLoop.md`.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::sleep;

use super::Shared;
use super::polls::kick_all;
use crate::core::{EngineAsk, EngineUp};
use crate::crash_loop;
use crate::log;
use crate::protocol::SafeMode;
use crate::wire;

/// How long a spawned engine gets to listen: JUCE and the audio device come up first.
const LISTEN_BOUND: Duration = Duration::from_secs(30);

/// The engine just up, told what this machine wants and asked what it is; then every poll runs at once.
pub fn spawn_inspect(shared: &Arc<Shared>, up: EngineUp) {
    let s = shared.clone();
    let generation = s.client.generation();
    tokio::spawn(async move {
        // A spawned engine is still loading when the core says it is up: its first answer comes when it listens.
        if !listening(&s, up.port, generation).await {
            return;
        }
        // The engine keeps its LAN closed until told: the musician's setting, every time it comes up.
        let lan = s.cx.settings.borrow().lan_open;
        s.client.set_lan_open(lan).await;
        if let Some(who) = s.client.whoami().await
            && s.cx.engine.borrow().up
        {
            judge(&s, up, &who);
        }
        // An engine gone or replaced meanwhile is another start's to answer.
        if s.client.generation() == generation {
            s.ready.store(true, Ordering::Release);
            kick_all(&s);
        }
    });
}

/// Waits for the engine's port to accept, at most `LISTEN_BOUND`; false when the engine was replaced or went meanwhile.
async fn listening(s: &Shared, port: u16, generation: u64) -> bool {
    let until = Instant::now() + LISTEN_BOUND;
    while s.client.generation() == generation {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        if Instant::now() > until {
            return false;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

fn text<'a>(o: &'a Value, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn judge(s: &Shared, up: EngineUp, who: &Value) {
    let safe = who.get("safeMode").filter(|m| m.is_object());
    // An engine older than the fields speaks protocol 1 and has no stamp, which no build of ours matches.
    let theirs = who.get("protocol").and_then(Value::as_i64).unwrap_or(1);
    let build = text(who, "build").unwrap_or_default();
    let foreign = theirs != wire::ENGINE_PROTOCOL || build != wire::build();
    // A new supervisor is a way out of safe mode, so the safe engine it finds goes too.
    if up.adopted && (foreign || safe.is_some()) && !s.replaced_adopted.swap(true, Ordering::AcqRel)
    {
        log::host(&if foreign {
            format!(
                "the engine already running speaks protocol {theirs}, build '{build}'; this supervisor speaks {}, build '{}' — replacing it",
                wire::ENGINE_PROTOCOL,
                wire::build()
            )
        } else {
            "the engine already running is in safe mode — replacing it with a normal one".into()
        });
        s.client.close();
        s.cx.ask(EngineAsk::Replace(
            "replacing the engine already running…".into(),
        ));
        return;
    }
    let Some(safe) = safe else { return };
    let mut mode = SafeMode {
        crashes: if up.adopted { 0 } else { up.safe_mode_crashes },
        suspect: String::new(),
    };
    if let Some(n) = safe.get("crashes").and_then(Value::as_i64) {
        mode.crashes = n.clamp(0, i64::from(u32::MAX)) as u32;
    }
    // The file it died loading is certain; the plugin it loaded last is only the likeliest.
    mode.suspect = ["diedLoading", "lastPlugin"]
        .iter()
        .filter_map(|k| text(safe, k))
        .find(|f| !f.is_empty())
        .unwrap_or_default()
        .to_string();
    log::host(&format!(
        "engine in safe mode{}",
        if mode.suspect.is_empty() {
            String::new()
        } else {
            format!("; the plugin it names is {}", mode.suspect)
        }
    ));
    s.cx.set_safe_mode(mode);
}

/// Kills an engine whose message thread is wedged (up, answering `whoami`, frozen for every session) and lets
/// the crash watch relaunch it.
pub fn spawn_hang_watch(shared: &Arc<Shared>) {
    let s = shared.clone();
    tokio::spawn(async move {
        loop {
            sleep(s.paces.hang).await;
            let watched = || s.up() && s.cx.engine.borrow().owns;
            if !watched() {
                continue;
            }
            let Some(who) = s.client.whoami().await else {
                continue;
            };
            // An engine older than the field reports none, and is never judged wedged.
            let Some(age) = who.get("messageThreadMs").and_then(Value::as_f64) else {
                continue;
            };
            if !watched() || !crash_loop::wedged(age) {
                continue;
            }
            log::host(&format!(
                "the engine's message thread has been silent for {} s — killing it, to be relaunched",
                (age / 1000.0) as i64
            ));
            // Killed, not stopped by the user: the core's crash watch counts the death and relaunches with its backoff.
            s.cx.ask(EngineAsk::Kill);
            s.client.close();
        }
    });
}
