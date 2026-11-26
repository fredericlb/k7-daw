//! registry part — this engine's listing at the studios: the heartbeat every 5 s while the engine is up, the
//! deregister on a stop or a quit, the registries the user added, paired or removed, and the performance
//! measured before the engine starts. Its slice of the snapshot is the registry line, the instance rows, the
//! last check and the heartbeat's light. What a reader gets wrong: one heartbeat is in flight at most, the
//! next tick skipping rather than stacking behind a dead network (`apps/k7-daw-backend/docs/engine/heartbeat.md`).

mod commands;
mod state;

use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::core::{Ack, Ctx, Event, PartSpec};
use crate::registry::Beat;
use state::State;

/// How often a heartbeat goes.
pub const HEARTBEAT: Duration = Duration::from_secs(5);
/// A hung registry must not hold the quit open.
const DEREGISTER_BOUND: Duration = Duration::from_millis(1500);

pub fn spec() -> PartSpec {
    PartSpec {
        name: "registry",
        commands: &[
            "weburi.add",
            "weburi.pair",
            "weburi.remove",
            "registry.deregister",
        ],
        start,
    }
}

/// What finished off the task: a heartbeat, or a hello.
enum Done {
    Beat(Beat, Option<Reply>),
    Probed(commands::Probed),
    Measured(Option<Value>, Ack),
}

/// The `result` a command waits for until a heartbeat has gone.
pub struct Reply {
    pub id: i64,
    pub ok: bool,
    pub message: String,
}

fn start(cx: Ctx, rx: mpsc::UnboundedReceiver<Event>) {
    tokio::spawn(run(cx, rx));
}

async fn run(cx: Ctx, mut rx: mpsc::UnboundedReceiver<Event>) {
    let (dtx, mut drx) = mpsc::unbounded_channel::<Done>();
    let mut st = State::new(cx.clone());
    let mut tick = interval(HEARTBEAT);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Boot(ack)) => boot(&mut st, &cx, &dtx, ack),
                Some(Event::EngineUp(up)) => {
                    st.safe = up.safe_mode_crashes > 0 && !up.adopted;
                    st.beat(&dtx, None);
                }
                Some(Event::EngineDown { user, ack }) => {
                    st.light = false;
                    st.publish();
                    if user {
                        st.leave_all().await;
                    }
                    if let Some(a) = ack {
                        let _ = a.send(());
                    }
                }
                Some(Event::Command(c)) => st.command(c, &dtx).await,
                Some(Event::Shutdown(ack)) => {
                    st.leave_all().await;
                    let _ = ack.send(());
                }
            },
            Some(done) = drx.recv() => match done {
                Done::Beat(beat, reply) => st.beat_done(beat, reply),
                Done::Probed(p) => st.probed(p, &dtx),
                Done::Measured(perf, ack) => {
                    st.performance = perf;
                    let _ = ack.send(());
                }
            },
            _ = tick.tick() => st.beat(&dtx, None),
        }
    }
}

/// Before the engine: the studios are resolved and their origins let in, and the machine is measured.
fn boot(st: &mut State, cx: &Ctx, dtx: &mpsc::UnboundedSender<Done>, ack: Ack) {
    st.load();
    crate::log::host(&format!(
        "registry: {} trusted roots, engine {}, presence {}",
        crate::trust::root_count(),
        cx.identity.id,
        st.presence(),
    ));
    let dir = cx.paths.studio_dir().to_path_buf();
    let dtx = dtx.clone();
    // Awaited by the core before the engine starts, so the CPU loop does not measure its plugin scan.
    tokio::spawn(async move {
        let m = tokio::task::spawn_blocking(move || crate::performance::measure(&dir)).await;
        let perf = m.ok().map(|m| {
            let perf = crate::performance::to_json(&m);
            crate::log::host(&format!(
                "performance {} {perf}",
                crate::performance::score(&m)
            ));
            perf
        });
        // The loop takes the measure in, then answers the core: no heartbeat goes without it.
        if let Err(mpsc::error::SendError(Done::Measured(_, ack))) =
            dtx.send(Done::Measured(perf, ack))
        {
            let _ = ack.send(());
        }
    });
}

/// `leave_all`, bounded: what a stop or a quit waits on.
pub(crate) async fn bounded<F: std::future::Future>(f: F) {
    let _ = timeout(DEREGISTER_BOUND, f).await;
}
