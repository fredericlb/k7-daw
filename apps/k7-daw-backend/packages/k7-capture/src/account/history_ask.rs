//! history_ask.rs — what the history routes ask of a workspace's own room (`docs/files/history.md`, "The routes"): the page,
//! a checkpoint, a diff and a restore.
//! What a reader gets wrong: a restore lands the files through the capture BEFORE it orders its entry, so its ledger learns
//! them; and a binary file is never part of a restore, the answer's `kept` naming each one left as it is.

use serde_json::{Value, json};

use super::{Account, lock, root_key};
use crate::failure::Failure;

/// One thing the routes ask of a room; the values are the query's or the body's, as given.
pub enum Ask {
    Page {
        before: Option<Value>,
        limit: Option<Value>,
    },
    Checkpoint {
        body: Value,
        by: String,
    },
    Diff {
        from: Option<Value>,
        to: Option<Value>,
    },
    Restore {
        body: Value,
    },
}

fn failure(status: u16, error: &Value) -> Failure {
    let code = match status {
        400 => "bad-request",
        404 => "no-history",
        410 => "gone",
        _ => "internal",
    };
    let said = error
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("it failed");
    Failure::new(status, code, said)
}

impl Account {
    /// The answer of `ask` over the history of the workspace at `root`.
    pub async fn history_ask(&self, root: &str, ask: Ask) -> Result<Value, Failure> {
        let a = self.attached_at(root).ok_or_else(|| {
            Failure::new(
                404,
                "unknown-workspace",
                "this workspace is not captured here",
            )
        })?;
        let unavailable = lock(&self.history_errors).get(&root_key(root)).cloned();
        if let Some(why) = unavailable {
            // The page says why; every other ask is refused with it.
            if matches!(ask, Ask::Page { .. }) {
                return Ok(json!({ "source": "device", "head": 0, "checkpoints": [],
                    "entries": [], "error": why }));
            }
            return Err(Failure::new(500, "history-unavailable", why));
        }
        let h = self.histories();
        let dir = a.log_dir.clone();
        let is_page = matches!(ask, Ask::Page { .. });
        let (status, mut body) = match ask {
            Ask::Restore { body } => self.restore_on_disk(&a, &body).await?,
            ask => {
                let (hh, d) = (h.clone(), dir.clone());
                tokio::task::spawn_blocking(move || {
                    // A room never written has no folder yet: made, so the page is an empty one.
                    let _ = hh.room(&d);
                    let relay = hh.relay();
                    match ask {
                        Ask::Page { before, limit } => k7_sync::history::page(
                            relay,
                            Some(&d),
                            "history",
                            before.as_ref(),
                            limit.as_ref(),
                        ),
                        Ask::Checkpoint { body, by } => {
                            k7_sync::history::add_checkpoint(relay, Some(&d), "history", &body, &by)
                        }
                        Ask::Diff { from, to } => k7_sync::history::diff(
                            relay,
                            Some(&d),
                            "history",
                            from.as_ref(),
                            to.as_ref(),
                        ),
                        Ask::Restore { .. } => (500, json!({})),
                    }
                })
                .await
                .unwrap_or((500, json!({ "error": "it failed" })))
            }
        };
        if status != 200 {
            return Err(failure(status, &body));
        }
        if is_page && let Some(m) = body.as_object_mut() {
            m.insert("source".into(), "device".into());
        }
        Ok(body)
    }

    /// The files at the point written to the disk through the capture, then one entry ordered for them in the room.
    async fn restore_on_disk(
        &self,
        a: &std::sync::Arc<super::Attached>,
        body: &Value,
    ) -> Result<(u16, Value), Failure> {
        let h = self.histories();
        let (relay, dir, b) = (h.relay().clone(), a.log_dir.clone(), body.clone());
        let (planned, kept) = tokio::task::spawn_blocking(move || plan(&relay, &dir, &b))
            .await
            .map_err(|e| Failure::new(500, "internal", e.to_string()))??;
        if !planned.is_empty() {
            let todo = planned.clone();
            let landed = a
                .handle
                .run(move |c| {
                    for (f, t) in &todo {
                        c.land(f, t.as_deref().map(str::as_bytes))?;
                    }
                    Ok::<(), std::io::Error>(())
                })
                .await;
            match landed {
                Some(Ok(())) => {}
                other => {
                    let why = match other {
                        Some(Err(e)) => e.to_string(),
                        _ => "its capture has stopped".to_string(),
                    };
                    self.settle_room(a, "restore stopped part way").await;
                    return Err(Failure::new(
                        500,
                        "internal",
                        format!("the restore stopped part way: {why}"),
                    ));
                }
            }
        }
        let (relay, dir, b, k) = (
            h.relay().clone(),
            a.log_dir.clone(),
            body.clone(),
            kept.clone(),
        );
        let (status, mut answer) = tokio::task::spawn_blocking(move || {
            k7_sync::history::restore_except(&relay, Some(&dir), "history", &b, &k)
        })
        .await
        .map_err(|e| Failure::new(500, "internal", e.to_string()))?;
        if let Some(m) = answer.as_object_mut().filter(|_| status == 200) {
            m.insert("kept".into(), json!(kept));
        }
        Ok((status, answer))
    }

    /// One entry named `label` for the files a git pull or merge landed through the capture, so undoing a pull is one step.
    /// A workspace this account does not capture has no history to write: nothing is done.
    pub async fn record_landing(&self, root: &str, label: &str) {
        if let Some(a) = self.attached_at(root) {
            self.settle_room(&a, label).await;
        }
    }

    /// The room made to say what the disk now holds, as one entry named `label`.
    async fn settle_room(&self, a: &std::sync::Arc<super::Attached>, label: &str) {
        let h = self.histories();
        let Some(Ok(files)) = a.handle.run(|c| c.texts()).await else {
            return;
        };
        let (dir, label) = (a.log_dir.clone(), label.to_string());
        let _ = tokio::task::spawn_blocking(move || h.reconcile(&dir, &files, &label)).await;
    }
}

/// A file a restore writes: its path, and the text it takes (None removes it).
type Change = (String, Option<String>);

/// The files a restore to the point changes, each with the text it takes (None removes it), and the binary files it leaves.
fn plan(
    relay: &k7_sync::relay::Relay,
    dir: &str,
    body: &Value,
) -> Result<(Vec<Change>, Vec<String>), Failure> {
    let bad = |m: &str| Failure::new(400, "bad-request", m);
    let room = relay
        .room(dir, "history")
        .map_err(|e| Failure::new(500, "internal", e))?;
    let l = room.lock();
    let seq = k7_sync::history::int_param(body.get("seq"))
        .filter(|q| *q <= l.room().head)
        .ok_or_else(|| bad("no such point"))?;
    let target = k7_sync::log::files_at(relay.js(), l.dir(), seq).map_err(|e| match e {
        k7_sync::log::LogError::Gap(m) => Failure::new(410, "gone", m),
        e => Failure::new(500, "internal", e.message().to_string()),
    })?;
    let now = l.room().files();
    let mut out = Vec::new();
    for (f, t) in &target {
        if now.get(f) != Some(t) {
            out.push((f.clone(), Some(t.clone())));
        }
    }
    for f in now.keys().filter(|f| !target.contains_key(*f)) {
        out.push((f.clone(), None));
    }
    // A blob ref is a text starting with NUL; its bytes are not kept here, so the file stays as it is.
    let blob = |t: Option<&String>| t.is_some_and(|t| t.starts_with('\u{0}'));
    let (binary, texts): (Vec<_>, Vec<_>) = out
        .into_iter()
        .partition(|(f, t)| blob(t.as_ref()) || blob(now.get(f)));
    Ok((texts, binary.into_iter().map(|(f, _)| f).collect()))
}
