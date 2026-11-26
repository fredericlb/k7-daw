//! jobs.rs — what a jobs channel asks of this machine: a table of handlers by request `type`, the context a
//! handler gets (fetch a file, ask whether a path goes back to the server) and the one job the agent answers
//! itself, `sync`. A handler runs as a task of its own, may reply as often as it has news, and may fail: the
//! caller then hears `error`. The table is built by `parts/tunnel/table.rs`, one line per job.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{Map, Value, json};

use super::agent::Shared;
use super::channel::{Channel, Payload};
use super::files::BoxFut;
use super::path_map::{PathMap, SyncProgress};

/// What a handler may ask of its channel beyond replying.
#[derive(Clone)]
pub struct JobContext {
    map: Option<Arc<PathMap>>,
}

impl JobContext {
    pub fn new(map: Option<Arc<PathMap>>) -> JobContext {
        JobContext { map }
    }

    /// Bring one file down by its LOCAL path, when a request names too many to fetch before it starts.
    pub async fn fetch(&self, local: &str) -> Result<bool, String> {
        match &self.map {
            Some(m) => m.ensure_local(local).await,
            None => Ok(false),
        }
    }

    /// Whether a LOCAL path lies inside a root this channel sends back to the server.
    pub fn writable(&self, local: &str) -> bool {
        self.map.as_ref().is_some_and(|m| m.writable(local))
    }
}

/// Sends one reply; any path in it goes back through the channel's map.
#[derive(Clone)]
pub struct Emit(Arc<dyn Fn(Value) + Send + Sync>);

impl Emit {
    pub fn new(f: impl Fn(Value) + Send + Sync + 'static) -> Emit {
        Emit(Arc::new(f))
    }

    pub fn send(&self, reply: Value) {
        (self.0)(reply)
    }
}

/// Answers one request; Err says why it failed, and the caller gets that as `error`.
pub type JobHandler =
    Arc<dyn Fn(Value, Emit, JobContext) -> BoxFut<'static, Result<(), String>> + Send + Sync>;

/// The jobs this machine does, by request `type`.
#[derive(Clone, Default)]
pub struct Jobs(BTreeMap<String, JobHandler>);

impl Jobs {
    pub fn add<F, Fut>(&mut self, name: &str, f: F)
    where
        F: Fn(Value, Emit, JobContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.0.insert(
            name.to_string(),
            Arc::new(move |req, emit, ctx| Box::pin(f(req, emit, ctx))),
        );
    }

    pub fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

impl Shared {
    /// Hand one request to its handler as a task of its own; every reply goes back through the map.
    pub fn run_job(self: &Arc<Self>, c: &Arc<Channel>, text: &str) {
        let Ok(Value::Object(req)) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let id = req.get("id").cloned().filter(|v| !v.is_null());
        let (sh, chan) = (self.clone(), c.clone());
        let emit = Emit::new(move |reply: Value| {
            let mut out = Map::new();
            if let Some(id) = &id {
                out.insert("id".into(), id.clone());
            }
            if let Value::Object(fields) = reply {
                out.extend(fields);
            }
            sh.to_hub(&chan, Payload::Text(Value::Object(out).to_string()));
        });
        let ty = req
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if ty == "sync" {
            self.run_sync(c, &req, emit);
            return;
        }
        let Some(handler) = self.opts.jobs.0.get(&ty).cloned() else {
            // Said here too: the server shows it to one person, the log keeps it for whoever looks at this machine.
            (self.opts.log)(&format!("refused job '{ty}': this machine does not do it"));
            emit.send(
                json!({"type": "error", "error": format!("this machine does not do '{ty}'")}),
            );
            return;
        };
        let (log, ctx) = (self.opts.log.clone(), JobContext::new(c.map.clone()));
        tokio::spawn(async move {
            let ran = tokio::spawn(handler(Value::Object(req), emit.clone(), ctx)).await;
            let why = match ran {
                Ok(Ok(())) => return,
                Ok(Err(e)) => e,
                Err(_) => "the job stopped unexpectedly".to_string(),
            };
            log(&format!("job '{ty}' failed: {why}"));
            emit.send(json!({"type": "error", "error": why}));
        });
    }

    /// Copy the listed files, one `syncing` reply per file copied, then `synced` (`engine-tunnel/docs/tunnel.md`).
    fn run_sync(self: &Arc<Self>, c: &Arc<Channel>, req: &Map<String, Value>, emit: Emit) {
        let locals: Vec<String> = req
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(str::to_string))
            .collect();
        let (log, map) = (self.opts.log.clone(), c.map.clone());
        tokio::spawn(async move {
            let started = Instant::now();
            let told = std::sync::atomic::AtomicBool::new(false);
            let report = |p: &SyncProgress| {
                if !told.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    log(&format!(
                        "sync: {} of {} file(s) to copy, {} MB",
                        p.files,
                        locals.len(),
                        p.bytes / (1024 * 1024)
                    ));
                }
                emit.send(json!({"type": "syncing", "files": p.files, "filesDone": p.files_done, "bytes": p.bytes, "bytesDone": p.bytes_done}));
            };
            let last = match &map {
                Some(m) => m.sync(&locals, &report).await,
                None => Ok(SyncProgress::default()),
            };
            match last {
                Ok(last) => {
                    log(&format!(
                        "sync: done in {} s, {} failed",
                        started.elapsed().as_secs(),
                        last.failures.len()
                    ));
                    emit.send(json!({"type": "synced", "failed": last.failures.len(), "failures": last.failures}));
                }
                Err(e) => {
                    log(&format!("job 'sync' failed: {e}"));
                    emit.send(json!({"type": "error", "error": e}));
                }
            }
        });
    }
}
