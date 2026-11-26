//! log.rs — the `log` job: a slice of the merged log, by byte offset, which the studio's Logs panel polls. The
//! reply's `raw` crosses the tunnel unmapped, since a log line may name a path of THIS machine.

use serde_json::json;

use crate::core::Ctx;
use crate::machine::log_slice::read_log;
use crate::tunnel::Jobs;

pub fn register(jobs: &mut Jobs, cx: &Ctx) {
    let paths = cx.paths.clone();
    jobs.add("log", move |req, emit, _| {
        let paths = paths.clone();
        async move {
            let offset = req["offset"]
                .as_f64()
                .filter(|o| *o >= 0.0)
                .map(|o| o as u64);
            let tail = req["tail"].as_f64().map(|t| t as i64);
            let raw = tokio::task::spawn_blocking(move || read_log(&paths, offset, tail))
                .await
                .map_err(|_| "the log could not be read".to_string())?;
            emit.send(json!({"type": "log", "raw": raw}));
            Ok(())
        }
    });
}
