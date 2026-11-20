//! host.rs — the calls the bundle's stand-ins make of the front: the registry's `bg.start` and `bg.end`, the machine ask
//! (2630, 2660). None takes the account's lock; an unknown kind is not ours and falls through.
//! What a reader gets wrong: a job's failure is an answer (`{jobError}`), not an error, as the TypeScript throws it.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use k7_file_service::runtime::ring::Level;
use serde_json::{Value, json};

use super::{Runtime, Slot};

impl Runtime {
    /// `bg.start`: false when the tag runs, else registered and listed at once.
    pub fn bg_start(&self, tag: &str) -> bool {
        let started = self.lock().jobs.start(tag, self.now());
        if started {
            self.counters.background.fetch_add(1, Ordering::Relaxed);
            super::report::totals()
                .background
                .fetch_add(1, Ordering::Relaxed);
        }
        started
    }

    /// `bg.end`: the job's end said in the ring as the TypeScript's `✓` or `✗` line; a second end says nothing.
    pub fn bg_end(&self, tag: &str, line: Option<&str>, error: Option<&str>) {
        let outcome = match error {
            Some(e) => Err(e.to_string()),
            None => Ok(line.unwrap_or(tag).to_string()),
        };
        let level = if outcome.is_ok() {
            Level::Info
        } else {
            Level::Error
        };
        let mut s = self.lock();
        if let Some(said) = s.jobs.end(tag, outcome, self.now()) {
            s.ring.push(level, &said, k7_hub::account::now_ms());
        }
    }

    /// A progress tell for a running job; it also renews the lease.
    pub fn bg_progress(&self, tag: &str, fraction: Option<f64>, detail: Option<&str>) {
        self.lock().jobs.progress(tag, fraction, detail, self.now());
    }

    /// Anything else heard while jobs run renews their lease (2631).
    pub fn heard(&self) {
        self.lock().jobs.heard(self.now());
    }

    /// A tell of ours, true when it was.
    pub fn tell(&self, kind: &str, args: &Value) -> bool {
        let text = |k: &str| args.get(k).and_then(Value::as_str);
        match kind {
            "bg.end" => self.bg_end(text("tag").unwrap_or(""), text("line"), text("error")),
            "bg.progress" => self.bg_progress(
                text("tag").unwrap_or(""),
                args.get("fraction").and_then(Value::as_f64),
                text("detail"),
            ),
            "log" => self.say(text("level").unwrap_or("info"), text("line").unwrap_or("")),
            "watch.moved" => self.watch.moved(),
            _ => return false,
        }
        true
    }

    /// A query or an ask of ours; `None` when the kind is not ours.
    pub async fn ask(self: &Arc<Self>, kind: &str, args: &Value) -> Option<Result<Value, String>> {
        match kind {
            "bg.start" => {
                let tag = args.get("tag").and_then(Value::as_str).unwrap_or("");
                Some(Ok(json!({ "value": self.bg_start(tag) })))
            }
            "jobs.ask" => {
                let last = args.get("last").and_then(Value::as_str).unwrap_or("");
                let request = args.get("request").cloned().unwrap_or(Value::Null);
                // The job holds its lease while the ask is in flight and at each reply (2631).
                let _flight = Slot::take(&self.flights, usize::MAX);
                self.heard();
                let me = self.clone();
                let heard: super::logs::Heard = Arc::new(move || me.heard());
                let reply = self.machine().ask(request, last.to_string(), heard).await;
                Some(Ok(match reply {
                    Ok(reply) => json!({ "value": reply }),
                    Err(sentence) => json!({ "jobError": sentence }),
                }))
            }
            "watch.join" => {
                let text = |k: &str| args.get(k).and_then(Value::as_str).map(String::from);
                let roots: Vec<String> = args
                    .get("roots")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let (me, dir, feed) = (self.clone(), text("dir").unwrap_or_default(), text("feed"));
                let _ = tokio::task::spawn_blocking(move || {
                    me.watch.join(&dir, &roots, feed.as_deref());
                })
                .await;
                Some(Ok(json!({ "value": true })))
            }
            "watch.still" => {
                let dir = args.get("dir").and_then(Value::as_str).unwrap_or("");
                Some(Ok(json!({ "value": self.watch.still(dir) })))
            }
            "watch.release" => {
                let under = args.get("under").and_then(Value::as_str).map(String::from);
                let me = self.clone();
                let n = tokio::task::spawn_blocking(move || me.watch.release(under.as_deref()))
                    .await
                    .unwrap_or(0);
                Some(Ok(json!({ "value": n })))
            }
            _ => None,
        }
    }
}
