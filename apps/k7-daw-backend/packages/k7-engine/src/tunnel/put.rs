//! put.rs — the uploads of one tunnel, and their bounds: chunks chained per request id so
//! they land in order; 16 open at once, 2 GiB each, 20 GiB an hour, 60 s without a chunk; a refused request's
//! later chunks ignored until its last. Holding a tunnel needs no account (`apps/k7-daw-frontend/packages/engine-tunnel/docs/tunnel.md`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexSet;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::frames::Root;
use super::hub_tunnel::Tunnel;
use super::uploads::Upload;

/// Request ids of refused uploads remembered, so their later chunks are not taken for a new upload.
const MAX_REFUSED: usize = 256;
const HOUR_MS: i64 = 3_600_000;

enum Step {
    Write(Vec<u8>, bool),
    Abort,
}

/// One upload under way: its ordered chain, its bytes so far, its idle timer.
pub(crate) struct Uploading {
    generation: u64,
    tx: mpsc::UnboundedSender<Step>,
    bytes: u64,
    idle: Option<JoinHandle<()>>,
}

impl Uploading {
    /// Its part file deleted, whatever it was doing.
    pub(crate) fn abort(self) {
        if let Some(i) = self.idle {
            i.abort();
        }
        let _ = self.tx.send(Step::Abort);
    }
}

#[derive(Default)]
pub(crate) struct Uploads {
    open: HashMap<i64, Uploading>,
    refused: IndexSet<i64>,
    hour: (i64, u64),
    generation: u64,
}

impl Uploads {
    pub(crate) fn drain(&mut self) -> Vec<Uploading> {
        self.open.drain().map(|(_, u)| u).collect()
    }

    /// Remembers a refused request id until its last chunk, the oldest forgotten first.
    fn remember(&mut self, req: i64, last: bool) {
        if last {
            return;
        }
        self.refused.insert(req);
        if self.refused.len() > MAX_REFUSED {
            self.refused.shift_remove_index(0);
        }
    }
}

impl Tunnel {
    pub(crate) fn on_put(
        self: &Arc<Self>,
        req: i64,
        roots: Arc<Vec<Root>>,
        path: String,
        last: bool,
        payload: Vec<u8>,
    ) {
        let limits = self.limits;
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let u = &mut s.uploads;
        if u.refused.contains(&req) {
            if last {
                u.refused.shift_remove(&req);
            }
            return;
        }
        if !u.open.contains_key(&req) {
            if u.open.len() >= limits.max_open {
                u.remember(req, last);
                drop(s);
                return self.put_error(req, "too many uploads open");
            }
            u.generation += 1;
            let generation = u.generation;
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(self.clone().chain(req, generation, roots, path, rx));
            u.open.insert(
                req,
                Uploading {
                    generation,
                    tx,
                    bytes: 0,
                    idle: None,
                },
            );
        }
        let n = payload.len() as u64;
        let held = u.open.get_mut(&req).expect("just made");
        held.bytes += n;
        let why = if held.bytes > limits.max_bytes {
            Some(format!("an upload is {} bytes at most", limits.max_bytes))
        } else {
            // The hourly budget of this tunnel's uploads.
            if now - u.hour.0 >= HOUR_MS {
                u.hour = (now, 0);
            }
            if u.hour.1 + n > limits.bytes_per_hour {
                Some(format!(
                    "{} bytes uploaded this hour already",
                    limits.bytes_per_hour
                ))
            } else {
                u.hour.1 += n;
                None
            }
        };
        let held = u.open.get_mut(&req).expect("just made");
        if let Some(why) = why {
            let generation = held.generation;
            drop(s);
            return self.fail(req, generation, last, &why);
        }
        if let Some(i) = held.idle.take() {
            i.abort();
        }
        let _ = held.tx.send(Step::Write(payload, last));
        if last {
            u.open.remove(&req);
        } else {
            let (t, generation) = (self.clone(), held.generation);
            held.idle = Some(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(limits.idle_ms)).await;
                t.fail(req, generation, false, "no chunk came in time");
            }));
        }
    }

    fn put_error(&self, req: i64, why: &str) {
        self.tell(&json!({ "t": "put-reply", "req": req, "error": why }));
    }

    /// Gives up an upload under way: its part file deleted, the agent told, its later chunks ignored.
    fn fail(&self, req: i64, generation: u64, last: bool, why: &str) {
        {
            let mut s = self.lock();
            let u = &mut s.uploads;
            // The idle timer calling this aborts itself too, harmlessly: nothing awaits after.
            if u.open.get(&req).is_some_and(|x| x.generation == generation)
                && let Some(x) = u.open.remove(&req)
            {
                x.abort();
            }
            u.remember(req, last);
        }
        (self.log)(&format!("{} upload {req}: {why}", self.engine_id));
        self.put_error(req, why);
    }

    /// The upload's chain: started, then each chunk written in the order it came.
    async fn chain(
        self: Arc<Self>,
        req: i64,
        generation: u64,
        roots: Arc<Vec<Root>>,
        path: String,
        mut rx: mpsc::UnboundedReceiver<Step>,
    ) {
        let mut up = match Upload::start(&roots, &path).await {
            Ok(u) => Some(u),
            Err(e) => {
                self.put_error(req, &e);
                None
            }
        };
        while let Some(step) = rx.recv().await {
            let Some(mut u) = up.take() else { continue };
            match step {
                Step::Abort => u.abort().await,
                Step::Write(payload, last) => {
                    let wrote = u.write(&payload).await;
                    match (wrote, last) {
                        (Ok(()), false) => up = Some(u),
                        (Ok(()), true) => match u.finish().await {
                            Ok(stat) => self.tell(
                                &json!({ "t": "put-reply", "req": req, "stat": stat.json() }),
                            ),
                            Err(e) => self.write_failed(req, generation, last, &e),
                        },
                        (Err(e), _) => {
                            u.abort().await;
                            self.write_failed(req, generation, last, &e);
                        }
                    }
                }
            }
        }
        if let Some(u) = up {
            u.abort().await;
        }
    }

    fn write_failed(&self, req: i64, generation: u64, last: bool, e: &str) {
        {
            let mut s = self.lock();
            let u = &mut s.uploads;
            if u.open.get(&req).is_some_and(|x| x.generation == generation)
                && let Some(x) = u.open.remove(&req)
                && let Some(i) = x.idle
            {
                i.abort();
            }
            u.remember(req, last);
        }
        self.put_error(req, e);
    }
}
