//! slot_jobs.rs — a server sets this machine's slots as tunnel jobs: `slots`, `slot-save`, `slot-enable`,
//! `slot-disable`, `slot-remove`, `slot-footswitch`, each answered with the store as it then stands
//! (`docs/slots.md`). What a reader gets wrong: a save copies only files under `<state>/remote/`, what the
//! tunnel brought down before the job ran, so a server never gets a file of this machine into a slot by naming it.

use std::sync::Arc;

use serde_json::Value;

use super::guard::mapped;
use crate::core::Ctx;
use crate::parts::slots::{Op, handle};
use crate::slots::Readable;
use crate::tunnel::{Emit, Jobs};

pub fn register(jobs: &mut Jobs, cx: &Ctx) {
    for ty in [
        "slots",
        "slot-save",
        "slot-enable",
        "slot-disable",
        "slot-remove",
        "slot-footswitch",
    ] {
        let cx = cx.clone();
        jobs.add(ty, move |req, emit, _| run(cx.clone(), ty, req, emit));
    }
}

/// A job names its slot by `slotId`: the job's own `id` is the channel's, matching a reply to its request.
fn slot_id(req: &Value) -> String {
    req["slotId"].as_str().unwrap_or("").to_string()
}

/// A name no other save of this machine holds at once.
fn nonce() -> String {
    let mut b = [0u8; 6];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

async fn run(cx: Ctx, ty: &'static str, req: Value, emit: Emit) -> Result<(), String> {
    let slots = handle(&cx.paths).ok_or("this machine keeps no slots")?;
    let op = match ty {
        "slots" => Op::List,
        "slot-save" => {
            let slot = req.get("slot").filter(|s| s.is_object());
            let slot = slot.ok_or("slot-save wants a slot")?;
            // Its paths are already this machine's: the tunnel fetched every file it names before the job ran.
            let staged = cx
                .paths
                .slots_dir()
                .join(format!("incoming-{}.json", nonce()));
            std::fs::create_dir_all(cx.paths.slots_dir()).map_err(|e| e.to_string())?;
            std::fs::write(&staged, slot.to_string()).map_err(|e| e.to_string())?;
            let cx = cx.clone();
            let readable: Readable = Arc::new(move |p| mapped(&cx, &p.to_string_lossy()));
            Op::Save {
                staged,
                readable: Some(readable),
            }
        }
        "slot-enable" => Op::Enable(slot_id(&req)),
        "slot-disable" => Op::Disable,
        "slot-remove" => Op::Remove(slot_id(&req)),
        _ => Op::Footswitch(slot_id(&req), req["cc"].as_i64()),
    };
    let staged = match &op {
        Op::Save { staged, .. } => Some(staged.clone()),
        _ => None,
    };
    let answer = slots.ask(op).await;
    if let Some(s) = staged {
        let _ = std::fs::remove_file(s);
    }
    emit.send(answer?.slots);
    Ok(())
}
