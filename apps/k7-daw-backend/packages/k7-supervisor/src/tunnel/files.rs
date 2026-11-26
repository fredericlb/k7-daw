//! files.rs — the agent's requests to the hub for a channel's files: stat, download, upload. A download lands
//! beside its target and is renamed over it, stamped with the server's mtime; replies arrive on the socket's
//! reader and wake the caller waiting on its request id. A dropped tunnel fails every request in flight.
//! What a reader gets wrong: a download fails when it goes QUIET, not when it is long.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};

use super::filetime;
use super::link::Link;
use super::wire::{CHUNK_BYTES, HIGH_WATER_BYTES, encode_binary};

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteStat {
    pub path: String,
    /// "file", "dir" or "missing".
    pub kind: String,
    pub size: Option<u64>,
    pub mtime_ms: Option<f64>,
}

impl RemoteStat {
    pub fn from_json(j: &Value) -> RemoteStat {
        RemoteStat {
            path: j["path"].as_str().unwrap_or("").to_string(),
            kind: j["kind"].as_str().unwrap_or("missing").to_string(),
            size: j["size"].as_f64().map(|n| n as u64),
            mtime_ms: j["mtimeMs"].as_f64(),
        }
    }
}

/// A download that arrived whole but whose target something holds open; `part` is left for the caller to place.
#[derive(Debug)]
pub enum FileError {
    InUse(String, PathBuf),
    Failed(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::InUse(m, _) | FileError::Failed(m) => f.write_str(m),
        }
    }
}

impl From<String> for FileError {
    fn from(m: String) -> Self {
        FileError::Failed(m)
    }
}

/// What the path map asks of one channel's files.
pub trait RemoteFiles: Send + Sync {
    fn stat(&self, paths: Vec<String>) -> BoxFut<'_, Result<Vec<RemoteStat>, String>>;
    fn download(&self, remote: RemoteStat, local: PathBuf) -> BoxFut<'_, Result<(), FileError>>;
    fn upload(&self, local: PathBuf, remote: String) -> BoxFut<'_, Result<RemoteStat, String>>;
}

/// How long the hub may take, and how quiet a download may go.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub reply: Duration,
    pub download_idle: Duration,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            reply: Duration::from_secs(300),
            download_idle: Duration::from_secs(120),
        }
    }
}

struct Download {
    file: tokio::sync::Mutex<Option<tokio::fs::File>>,
    done: Mutex<Option<oneshot::Sender<Result<(), String>>>>,
    last_chunk: Mutex<Instant>,
}

impl Download {
    fn finish(&self, r: Result<(), String>) {
        let tx = self.done.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(tx) = tx {
            let _ = tx.send(r);
        }
    }
}

#[derive(Default)]
struct Inner {
    next_req: i64,
    stats: HashMap<i64, oneshot::Sender<Vec<RemoteStat>>>,
    puts: HashMap<i64, oneshot::Sender<Result<RemoteStat, String>>>,
    gets: HashMap<i64, Arc<Download>>,
}

pub struct FileRequests {
    link: Arc<Link>,
    inner: Mutex<Inner>,
    pub bounds: Bounds,
}

/// A download that ends any way gives its request and its half file back.
struct Cleanup<'a> {
    reqs: &'a FileRequests,
    req: i64,
    part: PathBuf,
    /// A complete file the caller is to place stays.
    keep_part: bool,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        self.reqs.lock().gets.remove(&self.req);
        if !self.keep_part {
            let _ = std::fs::remove_file(&self.part);
        }
    }
}

fn part_of(local: &Path, req: i64) -> PathBuf {
    let mut p = local.as_os_str().to_owned();
    p.push(format!(".part-{req}"));
    PathBuf::from(p)
}

impl FileRequests {
    pub fn new(link: Arc<Link>) -> FileRequests {
        FileRequests {
            link,
            inner: Mutex::new(Inner {
                next_req: 1,
                ..Inner::default()
            }),
            bounds: Bounds::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn next(&self, inner: &mut Inner) -> i64 {
        let req = inner.next_req;
        inner.next_req += 1;
        req
    }

    pub async fn stat(&self, ch: i64, paths: Vec<String>) -> Result<Vec<RemoteStat>, String> {
        let (tx, rx) = oneshot::channel();
        let req = {
            let mut st = self.lock();
            let req = self.next(&mut st);
            st.stats.insert(req, tx);
            req
        };
        let msg = json!({"t": "stat", "req": req, "ch": ch, "paths": paths}).to_string();
        if !self.link.send_text(msg) {
            self.lock().stats.remove(&req);
            return Err("tunnel is down".into());
        }
        match timeout(self.bounds.reply, rx).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) => Err("tunnel dropped".into()),
            Err(_) => {
                self.lock().stats.remove(&req);
                Err("stat unanswered".into())
            }
        }
    }

    pub async fn download(
        &self,
        ch: i64,
        remote: &RemoteStat,
        local: &Path,
    ) -> Result<(), FileError> {
        if let Some(dir) = local.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let (tx, mut rx) = oneshot::channel();
        let req = {
            let mut st = self.lock();
            self.next(&mut st)
        };
        let part = part_of(local, req);
        let file = tokio::fs::File::create(&part)
            .await
            .map_err(|_| format!("cannot write {}", part.display()))?;
        let d = Arc::new(Download {
            file: tokio::sync::Mutex::new(Some(file)),
            done: Mutex::new(Some(tx)),
            last_chunk: Mutex::new(Instant::now()),
        });
        self.lock().gets.insert(req, d.clone());
        let mut cleanup = Cleanup {
            reqs: self,
            req,
            part: part.clone(),
            keep_part: false,
        };
        let msg = json!({"t": "get", "req": req, "ch": ch, "path": remote.path}).to_string();
        if !self.link.send_text(msg) {
            return Err("tunnel is down".to_string().into());
        }
        let tick = (self.bounds.download_idle / 4)
            .clamp(Duration::from_millis(10), Duration::from_secs(5));
        let outcome = loop {
            tokio::select! {
                r = &mut rx => break r,
                _ = sleep(tick) => {
                    let quiet = d.last_chunk.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
                    if quiet > self.bounds.download_idle {
                        return Err("download stalled".to_string().into());
                    }
                }
            }
        };
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => return Err("tunnel dropped".to_string().into()),
        }
        if std::fs::rename(&part, local).is_err() {
            // Windows will not rename over an open or read-only file; replace it in two steps instead.
            let _ = std::fs::remove_file(local);
            if let Err(e) = std::fs::rename(&part, local) {
                let held =
                    FileError::InUse(format!("{} is in use: {e}", local.display()), part.clone());
                cleanup.keep_part = true;
                return Err(held);
            }
        }
        filetime::set_mtime_ms(local, remote.mtime_ms.unwrap_or(0.0));
        Ok(())
    }

    pub async fn upload(&self, ch: i64, local: &Path, remote: &str) -> Result<RemoteStat, String> {
        let (tx, rx) = oneshot::channel();
        let req = {
            let mut st = self.lock();
            let req = self.next(&mut st);
            st.puts.insert(req, tx);
            req
        };
        if let Err(e) = self.send_file(ch, req, local, remote).await {
            self.lock().puts.remove(&req);
            return Err(e);
        }
        match timeout(self.bounds.reply, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("tunnel dropped".into()),
            Err(_) => {
                self.lock().puts.remove(&req);
                Err("upload unanswered".into())
            }
        }
    }

    async fn send_file(&self, ch: i64, req: i64, local: &Path, remote: &str) -> Result<(), String> {
        let send = |chunk: &[u8], last: bool| {
            let head = json!({"t": "put", "req": req, "ch": ch, "path": remote, "last": last});
            if self.link.send_binary(encode_binary(&head, chunk)) {
                Ok(())
            } else {
                Err("tunnel is down".to_string())
            }
        };
        let mut file = tokio::fs::File::open(local)
            .await
            .map_err(|_| format!("cannot read {}", local.display()))?;
        let size = file.metadata().await.map_err(|e| e.to_string())?.len();
        if size == 0 {
            return send(&[], true);
        }
        let (mut buf, mut sent) = (vec![0u8; CHUNK_BYTES], 0u64);
        while sent < size {
            // Never queue a whole take in memory behind a slow link.
            self.link.wait_below(HIGH_WATER_BYTES).await;
            let n = file.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            sent += n as u64;
            send(&buf[..n], sent >= size)?;
        }
        if sent < size {
            send(&[], true)?;
        }
        Ok(())
    }

    pub fn on_stat_reply(&self, req: i64, entries: &Value) {
        let Some(tx) = self.lock().stats.remove(&req) else {
            return;
        };
        let list = entries
            .as_array()
            .map_or_else(Vec::new, |a| a.iter().map(RemoteStat::from_json).collect());
        let _ = tx.send(list);
    }

    pub fn on_put_reply(&self, req: i64, stat: &Value, error: &str) {
        let Some(tx) = self.lock().puts.remove(&req) else {
            return;
        };
        let _ = tx.send(if stat.is_object() {
            Ok(RemoteStat::from_json(stat))
        } else {
            Err(if error.is_empty() {
                "upload refused"
            } else {
                error
            }
            .to_string())
        });
    }

    pub fn on_get_error(&self, req: i64, error: &str) {
        let Some(d) = self.lock().gets.remove(&req) else {
            return;
        };
        d.finish(Err(error.to_string()));
    }

    /// Written in arrival order, by the socket's reader.
    pub async fn on_chunk(&self, req: i64, last: bool, payload: &[u8]) {
        let d = {
            let mut st = self.lock();
            let Some(d) = st.gets.get(&req).cloned() else {
                return;
            };
            if last {
                st.gets.remove(&req);
            }
            d
        };
        *d.last_chunk.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        let mut slot = d.file.lock().await;
        let Some(file) = slot.as_mut() else {
            return;
        };
        let mut wrote = file.write_all(payload).await;
        if last && wrote.is_ok() {
            wrote = file.flush().await;
        }
        if wrote.is_err() {
            *slot = None;
            d.finish(Err("write failed".into()));
        } else if last {
            *slot = None;
            d.finish(Ok(()));
        }
    }

    /// The tunnel dropped: nothing in flight will ever be answered.
    pub fn fail_all(&self) {
        let mut st = self.lock();
        for (_, d) in st.gets.drain() {
            d.finish(Err("tunnel dropped".into()));
        }
        for (_, p) in st.puts.drain() {
            let _ = p.send(Err("tunnel dropped".into()));
        }
        for (_, s) in st.stats.drain() {
            let _ = s.send(Vec::new());
        }
    }
}

/// One channel's files, as the path map asks for them.
pub struct ChannelFiles {
    pub reqs: Arc<FileRequests>,
    pub ch: i64,
}

impl RemoteFiles for ChannelFiles {
    fn stat(&self, paths: Vec<String>) -> BoxFut<'_, Result<Vec<RemoteStat>, String>> {
        Box::pin(self.reqs.stat(self.ch, paths))
    }

    fn download(&self, remote: RemoteStat, local: PathBuf) -> BoxFut<'_, Result<(), FileError>> {
        Box::pin(async move { self.reqs.download(self.ch, &remote, &local).await })
    }

    fn upload(&self, local: PathBuf, remote: String) -> BoxFut<'_, Result<RemoteStat, String>> {
        Box::pin(async move { self.reqs.upload(self.ch, &local, &remote).await })
    }
}
