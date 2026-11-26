//! separate.rs — the `separate` job: split a mix with a model, both already on this disk, writing the stems where
//! the server takes them back. One reply per change of the cut, the last `separated`; the cut itself is M7's
//! (`separation::run`), keyed by its work, so a second request for the same cut joins the first.

use serde_json::{Value, json};

use super::guard::{require_mapped, require_writable};
use crate::core::Ctx;
use crate::separation;
use crate::tunnel::{Emit, JobContext, Jobs};

pub fn register(jobs: &mut Jobs, cx: &Ctx) {
    let cx = cx.clone();
    jobs.add("separate", move |req, emit, ctx| {
        run(cx.clone(), req, emit, ctx)
    });
}

fn text(req: &Value, key: &str) -> String {
    req[key].as_str().unwrap_or("").to_string()
}

async fn run(cx: Ctx, req: Value, emit: Emit, ctx: JobContext) -> Result<(), String> {
    let r = separation::Request {
        user: text(&req, "user"),
        source: text(&req, "source"),
        model: text(&req, "model"),
        out_dir: text(&req, "outDir"),
        outputs: req["outputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|o| o.as_str().map(str::to_string))
            .collect(),
        providers: cx.settings.borrow().onnx_providers.clone(),
    };
    require_writable(&ctx, &r.out_dir, "separate")?;
    require_mapped(&cx, &r.source, "separate")?;
    require_mapped(&cx, &r.model, "separate")?;
    tokio::task::spawn_blocking(move || {
        separation::run(r, |job| {
            let kind = if job["done"] == true {
                "separated"
            } else {
                "separating"
            };
            emit.send(json!({"type": kind, "job": job}));
        })
    })
    .await
    .map_err(|_| "the cut stopped unexpectedly".to_string())?
}
