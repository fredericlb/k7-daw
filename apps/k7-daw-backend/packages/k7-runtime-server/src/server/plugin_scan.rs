//! plugin_scan.rs — the one plugin scan of the process, its progress broadcast as `plugin-scan`, the catalogue as
//! `plugins`, and the detached work the scan and the probes run as (k7-runtime's `ServePlugins.cpp`).
//! What a reader gets wrong: the end is said AFTER the catalogue, a scan that throws included, since a gate waiting
//! on the end would otherwise wait for nothing; and a scan runs on a blocking task, never a socket's own.

use super::ctx::Ctx;
use super::scan_board::Status;
use crate::engine::plugins::PluginInfo;
use crate::log::say;
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::Instant;

/// When a scan runs: only on a cold cache, always (incremental, as at startup), or forced from scratch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    IfCold,
    Always,
    Force,
}

pub fn status_json(s: &Status) -> Value {
    json!({"type": "plugin-scan", "scanning": s.scanning, "done": s.done, "total": s.total,
           "current": s.current, "version": s.version})
}

/// The catalogue as a `plugins` reply carries it, narrowed by a format substring when `filter` names one.
pub fn plugins_json(ctx: &Ctx, plugins: &[PluginInfo], filter: &str) -> Value {
    let want = filter.to_ascii_lowercase();
    let list: Vec<Value> = plugins
        .iter()
        .filter(|p| want.is_empty() || p.format.to_ascii_lowercase().contains(&want))
        .map(|p| {
            json!({"name": p.name, "format": p.format, "manufacturer": p.manufacturer,
                   "instrument": p.instrument, "role": p.role, "category": p.category, "id": p.id})
        })
        .collect();
    // Whether a scan is running, so an asker knows this list is about to be replaced.
    json!({"type": "plugins", "plugins": list, "scanning": ctx.plugins.board.snapshot().scanning})
}

/// A problem that has to reach a human and not just stderr, as the engine's `warn()` does for no project.
pub fn warn(ctx: &Ctx, message: &str) {
    say!("warning: {message}");
    ctx.broadcast(&json!({"type": "engine-warning", "subject": "", "message": message}));
}

/// Runs `work` on a blocking task that the server may abandon at exit; a panic is said as a warning, not lost.
pub fn detached(ctx: &Arc<Ctx>, work: impl FnOnce() + Send + 'static) {
    let ctx = ctx.clone();
    ctx.plugins.detached.fetch_add(1, Ordering::SeqCst);
    tokio::task::spawn_blocking(move || {
        let outcome = catch_unwind(AssertUnwindSafe(work));
        ctx.plugins.detached.fetch_sub(1, Ordering::SeqCst);
        if let Err(p) = outcome {
            let why = p
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| p.downcast_ref::<&str>().copied());
            match why {
                Some(w) => warn(&ctx, &format!("background work failed: {w}")),
                None => warn(&ctx, "background work failed (unknown exception)"),
            }
        }
    });
}

/// One serialized scan for the whole process; `announce` hears the catalogue it ended on BEFORE the end is said.
/// `Err` is why a scan threw, after the end was said anyway.
pub fn ensure_scanned(
    ctx: &Ctx,
    when: When,
    announce: &dyn Fn(&[PluginInfo]),
) -> Result<Vec<PluginInfo>, String> {
    let api = ctx.engine.api();
    let _one_at_a_time = ctx
        .plugins
        .scan_lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if when == When::IfCold && api.plugin_cache_warm() {
        // Another caller already warmed it.
        let cached = api.load_plugin_cache();
        announce(&cached);
        return Ok(cached);
    }
    ctx.broadcast(&status_json(&ctx.plugins.board.begin(Instant::now())));
    let mut progress = |done: i32, total: i32, loading: &str| {
        if let Some(now) = ctx.plugins.board.step(done, total, loading, Instant::now()) {
            ctx.broadcast(&status_json(&now));
        }
    };
    match api.scan_plugins(when == When::Force, &mut progress) {
        Ok(found) => {
            // The catalogue first, then the end: whoever re-reads on the end finds the new list already sent.
            let ended = ctx.plugins.board.finish();
            announce(&found);
            ctx.broadcast(&status_json(&ended));
            Ok(found)
        }
        Err(why) => {
            let ended = ctx.plugins.board.finish();
            announce(&api.load_plugin_cache());
            ctx.broadcast(&status_json(&ended));
            Err(why)
        }
    }
}

/// `ensure_scanned` as detached work: a scan that threw is warned.
pub fn scan_in_background(
    ctx: &Arc<Ctx>,
    when: When,
    announce: impl Fn(&[PluginInfo]) + Send + 'static,
) {
    let c = ctx.clone();
    detached(ctx, move || {
        if let Err(why) = ensure_scanned(&c, when, &announce) {
            warn(&c, &format!("background work failed: {why}"));
        }
    });
}

/// The incremental scan at EVERY start, in the background: a plugin installed or removed since the last run is
/// found without anyone asking, and every socket hears the new list.
pub fn start(ctx: &Arc<Ctx>) {
    let c = ctx.clone();
    detached(ctx, move || {
        let warm = c.engine.api().plugin_cache_warm();
        say!(
            "{}",
            if warm {
                "[plugins] startup scan — only new or changed files are loaded"
            } else {
                "[plugins] cache cold at startup — scanning in the background, this can take a while"
            }
        );
        let said = c.clone();
        let announce = move |found: &[PluginInfo]| said.broadcast(&plugins_json(&said, found, ""));
        match ensure_scanned(&c, When::Always, &announce) {
            Ok(_) => say!("[plugins] startup scan complete"),
            Err(why) => warn(&c, &format!("background work failed: {why}")),
        }
    });
}
