//! parts/separation.rs — the cut's part: it answers no command and owns no state, since a cut is a thread of its
//! own (`separation::run`); it ends the cuts in progress when the supervisor goes, so a provider that hangs cannot
//! hold the quit open.

use tokio::sync::mpsc;

use crate::core::{Ctx, Event, PartSpec};
use crate::separation;

fn start(_cx: Ctx, mut rx: mpsc::UnboundedReceiver<Event>) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                Event::Boot(ack) => {
                    let _ = ack.send(());
                }
                Event::Shutdown(ack) => {
                    separation::terminate_all();
                    let _ = ack.send(());
                }
                Event::EngineDown { ack: Some(ack), .. } => {
                    let _ = ack.send(());
                }
                _ => {}
            }
        }
    });
}

pub fn spec() -> PartSpec {
    PartSpec {
        name: "separation",
        commands: &[],
        start,
    }
}
