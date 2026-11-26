//! settings.rs — the two jobs that are settings, not work: `onnx-providers` and `chains` answer in one frame and
//! write the machine's own file on the way (`stemSplit.md`), so the studio's card changes what this machine
//! separates with. They go through `Ctx::update_settings`, the core being the file's one writer.

use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio::time::timeout;

use crate::core::Ctx;
use crate::settings::{Settings, normalise_providers};
use crate::tunnel::Jobs;

/// Longest a job waits to see a change it asked of the core.
const SEEN: Duration = Duration::from_secs(1);

/// The chains as a `chains` reply names them: a key only when this machine has one.
fn chains_of(s: &Settings) -> Value {
    let mut chains = Map::new();
    if !s.stems_chain.is_empty() {
        chains.insert("stems".into(), json!(s.stems_chain));
    }
    if !s.instrumental_chain.is_empty() {
        chains.insert("instrumental".into(), json!(s.instrumental_chain));
    }
    Value::Object(chains)
}

/// The keys `set` names written; a key it does not name is left alone, and a blank name clears one.
fn put_chains(s: &mut Settings, set: &Value) {
    let put = |key: &str, into: &mut String| {
        if let Some(v) = set.get(key).and_then(Value::as_str) {
            *into = if v.trim().is_empty() {
                String::new()
            } else {
                v.to_string()
            };
        }
    };
    put("stems", &mut s.stems_chain);
    put("instrumental", &mut s.instrumental_chain);
}

pub fn register(jobs: &mut Jobs, cx: &Ctx) {
    let cx1 = cx.clone();
    // What this machine separates on; given `set`, written first: the server's `system onnx` asks here.
    jobs.add("onnx-providers", move |req, emit, _| {
        let cx = cx1.clone();
        async move {
            if req.get("set").is_some_and(Value::is_array) {
                let want = normalise_providers(req.get("set"));
                let w = want.clone();
                cx.update_settings(move |s| s.onnx_providers = w);
                let mut seen = cx.settings.clone();
                let _ = timeout(SEEN, seen.wait_for(|s| s.onnx_providers == want)).await;
            }
            let providers = cx.settings.borrow().onnx_providers.clone();
            emit.send(json!({"type": "onnx-providers", "providers": providers}));
            Ok(())
        }
    });
    let cx2 = cx.clone();
    // The chains this machine separates with; given `set`, the keys it names are written first.
    jobs.add("chains", move |req, emit, _| {
        let cx = cx2.clone();
        async move {
            if let Some(set) = req.get("set").filter(|s| s.is_object()).cloned() {
                let mut after = cx.settings.borrow().clone();
                put_chains(&mut after, &set);
                let want = chains_of(&after);
                cx.update_settings(move |s| put_chains(s, &set));
                let mut seen = cx.settings.clone();
                let _ = timeout(SEEN, seen.wait_for(|s| chains_of(s) == want)).await;
            }
            let chains = chains_of(&cx.settings.borrow());
            emit.send(json!({"type": "chains", "chains": chains}));
            Ok(())
        }
    });
}
