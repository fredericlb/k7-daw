//! files.rs — the two jobs about files kept for a server: `prefetch` counts a set list the channel brought down
//! ahead of a gig, and `send-up` stamps an undo's audio so the channel uploads it (`apps/k7-daw-backend/docs/files/history.md`).
//! What a reader gets wrong: a file that is no copy of a server's answers as an absent one, so a probe learns nothing.

use std::path::Path;
use std::time::SystemTime;

use serde_json::json;

use super::guard::{mapped, require_writable};
use crate::core::Ctx;
use crate::tunnel::Jobs;

pub fn register(jobs: &mut Jobs, cx: &Ctx) {
    let cx1 = cx.clone();
    // The set list's records, already brought down by the channel before this runs, so this only counts.
    jobs.add("prefetch", move |req, emit, _| {
        let cx = cx1.clone();
        async move {
            let count = req["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str())
                .filter(|f| mapped(&cx, f) && Path::new(f).exists())
                .count();
            emit.send(json!({"type": "prefetched", "count": count}));
            Ok(())
        }
    });
    // Send this machine's copy of a file BACK up: an undo's audio, which only this machine keeps.
    jobs.add("send-up", move |req, emit, ctx| async move {
        let file = req["file"].as_str().unwrap_or("").to_string();
        require_writable(&ctx, &file, "send-up")?;
        if file.is_empty() || !Path::new(&file).is_file() {
            return Err("not kept on this machine".into());
        }
        // A fresh mtime is what makes the channel upload a file it last brought DOWN, and it counts as a use.
        if let Ok(f) = std::fs::File::options().write(true).open(&file) {
            let _ = f.set_modified(SystemTime::now());
        }
        emit.send(json!({"type": "sent", "file": file}));
        Ok(())
    });
}
