//! render.rs — the `render` job: bounce a ProjectDoc through THIS machine's engine, `out` taking the mix alone
//! and `outDir` the whole manifest (`apps/k7-daw-backend/docs/engine/tunnelJobs.md`). The work is
//! `machine::render`; this checks the paths first and counts the renders in flight on the snapshot, which the
//! app's Busy reads. What a reader gets wrong: the count rides a patch of this part, so ONE counter serves
//! every agent.

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::guard::require_writable;
use crate::core::Ctx;
use crate::engine::{IN_APP, find_binary};
use crate::machine::render::{Request, Via, render};
use crate::tunnel::Jobs;

/// The renders in flight, as the snapshot's `rendering`.
pub struct Rendering {
    cx: Ctx,
    n: Mutex<u32>,
}

impl Rendering {
    pub fn new(cx: &Ctx) -> Arc<Rendering> {
        Arc::new(Rendering {
            cx: cx.clone(),
            n: Mutex::new(0),
        })
    }

    /// A render started (+1) or ended (-1); never below zero.
    pub fn add(&self, delta: i32) {
        let mut n = self.n.lock().unwrap_or_else(|e| e.into_inner());
        *n = n.saturating_add_signed(delta);
        let now = *n;
        self.cx.patch(false, move |s| s.rendering = now);
    }
}

pub fn register(jobs: &mut Jobs, cx: &Ctx, rendering: &Arc<Rendering>) {
    let (cx, rendering) = (cx.clone(), rendering.clone());
    jobs.add("render", move |req, emit, ctx| {
        let (cx, rendering) = (cx.clone(), rendering.clone());
        async move {
            let text = |k: &str| req[k].as_str().unwrap_or("").to_string();
            let (out, out_dir) = (text("out"), text("outDir"));
            if (out.is_empty() && out_dir.is_empty()) || !req["doc"].is_object() {
                return Err("render: a doc and an out path or folder are required".into());
            }
            require_writable(&ctx, if out.is_empty() { &out_dir } else { &out }, "render")?;
            let via = if IN_APP {
                let r = rendering.clone();
                Via::Engine {
                    port: cx.engine.borrow().port,
                    scratch_under: cx.paths.studio_dir().join("render-tmp"),
                    rendering: Arc::new(move |d| r.add(d)),
                }
            } else {
                let exe = std::env::current_exe().ok();
                let over = std::env::var_os("STUDIO_ENGINE").map(std::path::PathBuf::from);
                let bin = find_binary(over.as_deref(), exe.as_deref());
                Via::Child(bin.ok_or("no k7-runtime binary on this machine to render with")?)
            };
            let req = Request {
                doc: req["doc"].clone(),
                out,
                out_dir,
                stems: req["stems"] == true,
            };
            let (e2, w) = (emit.clone(), ctx.clone());
            let started = move || e2.send(json!({"type": "rendering"}));
            let reply = render(&req, &via, &move |p| w.writable(p), &started).await?;
            emit.send(reply);
            Ok(())
        }
    });
}
