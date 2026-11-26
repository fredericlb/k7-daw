//! instruments part — the sampled instruments this machine keeps (`docs/instruments.md`): the Downloads rows and
//! the one status line of the snapshot, the queue that fetches one at a time in the order asked, and the
//! commands `instruments.download` and `instruments.remove`. The first boot settles what the machine keeps, and
//! the fetch of what it keeps starts once the engine is up, so the performance measure of the boot runs clean.
//! What a reader gets wrong: a running download is never cut, so its removal is refused with a reason.

mod state;

use std::time::Duration;

use tokio::sync::mpsc;

use crate::core::{Ctx, Event, PartSpec};
use crate::instruments::{self, Instrument};
use state::State;

/// The longest the transfer waits for its next byte, a connect or an answer: a stalled line fails and resumes next time.
const IDLE: Duration = Duration::from_secs(60);

pub fn spec() -> PartSpec {
    PartSpec {
        name: "instruments",
        commands: &["instruments.download", "instruments.remove"],
        start,
    }
}

fn start(cx: Ctx, rx: mpsc::UnboundedReceiver<Event>) {
    tokio::spawn(run(cx, rx, instruments::all().to_vec(), IDLE));
}

/// What a worker tells the part: a download's news, its end, or a removal's.
pub(crate) enum Done {
    Progress(String, instruments::Progress),
    Fetched(String, Result<(), String>),
    Removed {
        id: i64,
        name: String,
        error: String,
    },
}

/// The part's task over `catalogue`, which the tests replace with archives served on 127.0.0.1.
pub async fn run(
    cx: Ctx,
    mut rx: mpsc::UnboundedReceiver<Event>,
    catalogue: Vec<Instrument>,
    idle: Duration,
) {
    let (dtx, mut drx) = mpsc::unbounded_channel::<Done>();
    let mut st = State::new(cx, catalogue, idle, dtx);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Boot(ack)) => {
                    st.boot();
                    let _ = ack.send(());
                }
                Some(Event::EngineUp(_)) => st.engine_up(),
                Some(Event::EngineDown { ack, .. }) => {
                    if let Some(a) = ack {
                        let _ = a.send(());
                    }
                }
                Some(Event::Command(c)) => st.command(c),
                Some(Event::Shutdown(ack)) => {
                    st.abort();
                    let _ = ack.send(());
                }
            },
            Some(done) = drx.recv() => st.done(done),
        }
    }
}
