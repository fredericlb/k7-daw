//! path_map.rs — the engine names files by LOCAL path and the server by its own; this rewrites every JSON string
//! under a shared root as a frame crosses, and moves the file FIRST: downloaded before a frame naming it reaches
//! the engine, uploaded before a frame naming a file the engine wrote reaches the server. Whole strings only,
//! never substrings. A channel the server syncs itself only rewrites. The twin of `engine-tunnel/pathMap.ts`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use serde_json::Value;

use super::files::{FileError, RemoteFiles, RemoteStat};
use super::filetime;
use super::ledger::{SyncLedger, Synced};
use super::segments::{join_under, relative_under};

/// Frames at meter rate carry no path, and pass through untouched.
const UNMAPPED_TYPES: [&str; 3] = ["state", "io-levels", "levels"];
/// Files fetched or copied at once: a project can name hundreds, and each download is a request in flight.
const PARALLEL: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct MappedRoot {
    /// The server's absolute POSIX path.
    pub remote: String,
    /// Where this machine keeps its copy, in the local path style.
    pub local: String,
    pub writable: bool,
}

/// How far a `sync` has got: only the files it had to copy count, not those already here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncProgress {
    pub files: u64,
    pub files_done: u64,
    pub bytes: u64,
    pub bytes_done: u64,
    /// One line per file that did not arrive: its name, then why, never a path, which the map would upload.
    pub failures: Vec<String>,
}

pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

pub struct PathMap {
    roots: Vec<MappedRoot>,
    remote_needles: Vec<String>,
    local_needles: Vec<String>,
    files: Box<dyn RemoteFiles>,
    ledger: Arc<SyncLedger>,
    log: Log,
}

/// A string as it appears inside a JSON document, quotes stripped: what a raw-text search must look for.
fn json_escaped(s: &str) -> String {
    let q = serde_json::to_string(s).unwrap_or_default();
    q[1..q.len() - 1].to_string()
}

fn map_strings(v: &Value, f: &dyn Fn(&str) -> Option<String>) -> Value {
    match v {
        Value::String(s) => f(s).map_or_else(|| v.clone(), Value::String),
        Value::Array(a) => Value::Array(a.iter().map(|x| map_strings(x, f)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), map_strings(x, f)))
                .collect(),
        ),
        _ => v.clone(),
    }
}

fn collect(v: &Value, keep: &dyn Fn(&str) -> bool, into: &mut BTreeSet<String>) {
    match v {
        Value::String(s) if keep(s) => {
            into.insert(s.clone());
        }
        Value::Array(a) => a.iter().for_each(|x| collect(x, keep, into)),
        Value::Object(o) => o.values().for_each(|x| collect(x, keep, into)),
        _ => {}
    }
}

/// `clip.wav` → `clip~<server mtime>.wav`: one name per version, so a version placed once is found after a restart.
fn beside(local: &str, remote_mtime: f64) -> String {
    let p = Path::new(local);
    let mut name = p.file_stem().unwrap_or_default().to_os_string();
    name.push(format!("~{}", remote_mtime as i64));
    if let Some(ext) = p.extension() {
        name.push(".");
        name.push(ext);
    }
    p.with_file_name(name).to_string_lossy().into_owned()
}

impl PathMap {
    pub fn new(
        roots: Vec<MappedRoot>,
        files: Box<dyn RemoteFiles>,
        ledger: Arc<SyncLedger>,
        log: Log,
    ) -> PathMap {
        PathMap {
            remote_needles: roots.iter().map(|r| json_escaped(&r.remote)).collect(),
            local_needles: roots.iter().map(|r| json_escaped(&r.local)).collect(),
            roots,
            files,
            ledger,
            log,
        }
    }

    /// The fixed local spelling of a server path, which keys the ledger whatever copy is current.
    fn canonical(&self, remote: &str) -> Option<String> {
        for r in &self.roots {
            if remote == r.remote {
                return Some(r.local.clone());
            }
            if let Some(rest) = remote.strip_prefix(&format!("{}/", r.remote)) {
                return join_under(&r.local, rest);
            }
        }
        None
    }

    fn actual(&self, canonical: &str) -> String {
        self.ledger
            .redirect(canonical)
            .unwrap_or_else(|| canonical.to_string())
    }

    /// The local spelling of a server path under a shared root.
    pub fn to_local(&self, remote: &str) -> Option<String> {
        let c = self.canonical(remote)?;
        Some(self.ledger.redirect(&c).unwrap_or(c))
    }

    /// The server spelling of a local path under a shared root, with its root.
    pub fn to_remote(&self, named: &str) -> Option<(String, &MappedRoot)> {
        // A version placed beside a held file stands for the canonical path, which is what the server knows.
        let local = self
            .ledger
            .canonical_of(named)
            .unwrap_or_else(|| named.to_string());
        self.roots.iter().find_map(|r| {
            let rel = relative_under(&r.local, &local)?;
            let remote = if rel.is_empty() {
                r.remote.clone()
            } else {
                format!("{}/{rel}", r.remote)
            };
            Some((remote, r))
        })
    }

    /// Whether a LOCAL path lies inside a root this channel sends back to the server.
    pub fn writable(&self, local: &str) -> bool {
        self.to_remote(local).is_some_and(|(_, r)| r.writable)
    }

    fn place(&self, canonical: &str, actual: &str) {
        let before = self.ledger.place(canonical, actual);
        // The copy it replaces goes when nothing holds it any more; one still held stays until a later try.
        if let Some(b) = before.filter(|b| b != actual) {
            let _ = std::fs::remove_file(b);
        }
    }

    /// Bring ONE file down by its local path; false when the server has no such file.
    pub async fn ensure_local(&self, local: &str) -> Result<bool, String> {
        let Some((remote, _)) = self.to_remote(local) else {
            return Ok(false);
        };
        let entries = self.files.stat(vec![remote]).await?;
        let Some(e) = entries.first().filter(|e| e.kind == "file") else {
            return Ok(false);
        };
        let canonical = self
            .ledger
            .canonical_of(local)
            .unwrap_or_else(|| local.to_string());
        self.ensure_file(e, &canonical).await?;
        Ok(true)
    }

    /// A server→engine frame, rewritten; with `fetch`, every file it names present locally first.
    pub async fn outbound(&self, text: &str, fetch: bool) -> Result<String, String> {
        if !self
            .remote_needles
            .iter()
            .any(|n| text.contains(n.as_str()))
        {
            return Ok(text.to_string());
        }
        let Ok(msg) = serde_json::from_str::<Value>(text) else {
            return Ok(text.to_string());
        };
        let ty = msg["type"].as_str().unwrap_or("").to_string();
        if UNMAPPED_TYPES.contains(&ty.as_str()) {
            return Ok(text.to_string());
        }
        // A `sync` request names the files it is about to copy itself, reporting as it goes.
        if fetch && ty != "sync" {
            let mut named = BTreeSet::new();
            collect(&msg, &|s| self.canonical(s).is_some(), &mut named);
            // A whole document is checked against the server; a delta trusts what an earlier one settled.
            let recheck = ty == "project";
            let ask: Vec<String> = named
                .into_iter()
                .filter(|p| {
                    recheck
                        || self
                            .canonical(p)
                            .is_none_or(|c| self.ledger.get(&c).is_none())
                })
                .collect();
            if !ask.is_empty() {
                (self.log)(&format!(
                    "checking {} file(s) before a '{ty}' frame",
                    ask.len()
                ));
                self.bring_down(self.files.stat(ask).await?).await;
            }
        }
        Ok(map_strings(&msg, &|s| self.to_local(s)).to_string())
    }

    /// An engine→server frame, rewritten, every file the engine changed uploaded first.
    pub async fn inbound(&self, text: &str) -> String {
        if !self.local_needles.iter().any(|n| text.contains(n.as_str())) {
            return text.to_string();
        }
        let Ok(mut msg) = serde_json::from_str::<Value>(text) else {
            return text.to_string();
        };
        // A reply's `raw` is delivered as the machine wrote it: its caller needs the machine's own spelling.
        let raw = msg.as_object_mut().and_then(|o| o.shift_remove("raw"));
        let mut named = BTreeSet::new();
        collect(&msg, &|s| self.to_remote(s).is_some(), &mut named);
        for local in &named {
            self.send_up(local).await;
        }
        let mut mapped = map_strings(&msg, &|s| self.to_remote(s).map(|(r, _)| r));
        if let (Some(raw), Some(o)) = (raw, mapped.as_object_mut()) {
            o.insert("raw".into(), raw);
        }
        mapped.to_string()
    }

    /// Bring every listed LOCAL path up to date, reporting after each copy; returns the last report.
    pub async fn sync(
        &self,
        locals: &[String],
        progress: &(dyn Fn(&SyncProgress) + Sync),
    ) -> Result<SyncProgress, String> {
        let ask: Vec<String> = locals
            .iter()
            .filter_map(|l| self.to_remote(l).map(|m| m.0))
            .collect();
        let mut p = SyncProgress::default();
        let mut todo: Vec<RemoteStat> = Vec::new();
        if !ask.is_empty() {
            for e in self.files.stat(ask).await? {
                let Some(local) = self.canonical(&e.path) else {
                    continue;
                };
                match e.kind.as_str() {
                    "dir" => drop(std::fs::create_dir_all(&local)),
                    "missing" => {
                        if let Some(dir) = Path::new(&local).parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                    }
                    _ if !self.up_to_date(&e, &local) => {
                        p.bytes += e.size.unwrap_or(0);
                        todo.push(e);
                    }
                    _ => {}
                }
            }
        }
        p.files = todo.len() as u64;
        progress(&p);
        let shared = Mutex::new(p);
        futures_util::stream::iter(todo.iter())
            .for_each_concurrent(PARALLEL, |e| async {
                let canonical = self.canonical(&e.path).unwrap_or_default();
                let failure = match self.ensure_file(e, &canonical).await {
                    Ok(()) => None,
                    Err(err) => {
                        (self.log)(&format!("could not bring down {}: {err}", e.path));
                        let name = e.path.rsplit('/').next().unwrap_or("");
                        Some(format!("{name} — {err}"))
                    }
                };
                let mut p = shared.lock().unwrap_or_else(|x| x.into_inner());
                p.files_done += 1;
                p.bytes_done += e.size.unwrap_or(0);
                p.failures.extend(failure);
                progress(&p);
            })
            .await;
        Ok(shared.into_inner().unwrap_or_else(|x| x.into_inner()))
    }

    async fn bring_down(&self, entries: Vec<RemoteStat>) {
        futures_util::stream::iter(entries.iter())
            .for_each_concurrent(PARALLEL, |e| async {
                let Some(local) = self.canonical(&e.path) else {
                    return;
                };
                match e.kind.as_str() {
                    "dir" => drop(std::fs::create_dir_all(&local)),
                    "missing" => {
                        if let Some(dir) = Path::new(&local).parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                    }
                    _ => {
                        if let Err(err) = self.ensure_file(e, &local).await {
                            (self.log)(&format!("could not bring down {}: {err}", e.path));
                        }
                    }
                }
            })
            .await;
    }

    fn up_to_date(&self, e: &RemoteStat, local: &str) -> bool {
        let size = e.size.unwrap_or(0);
        let remote_mtime = e.mtime_ms.unwrap_or(0.0);
        let have = filetime::stat(Path::new(&self.actual(local)));
        let known = self.ledger.get(local);
        let same = match (known, have) {
            (Some(k), Some(h)) => {
                k.size == size
                    && filetime::same(k.remote_mtime_ms, remote_mtime)
                    && h.size == size
                    && filetime::same(h.mtime_ms, k.local_mtime_ms)
            }
            (Some(_), None) => false,
            (None, Some(h)) => h.size == size && filetime::same(h.mtime_ms, remote_mtime),
            (None, None) => false,
        };
        if let (true, None, Some(h)) = (same, known, have) {
            self.ledger.set(
                local,
                Synced {
                    size,
                    local_mtime_ms: h.mtime_ms,
                    remote_mtime_ms: remote_mtime,
                },
            );
        }
        same
    }

    async fn ensure_file(&self, e: &RemoteStat, local: &str) -> Result<(), String> {
        let size = e.size.unwrap_or(0);
        let remote_mtime = e.mtime_ms.unwrap_or(0.0);
        if self.up_to_date(e, local) {
            return Ok(());
        }
        let aside = beside(local, remote_mtime);
        // A version already placed beside a held file, before a restart say, is adopted rather than fetched again.
        if let Some(there) = filetime::stat(Path::new(&aside))
            .filter(|t| t.size == size && filetime::same(t.mtime_ms, remote_mtime))
        {
            self.place(local, &aside);
            self.ledger.set(
                local,
                Synced {
                    size,
                    local_mtime_ms: there.mtime_ms,
                    remote_mtime_ms: remote_mtime,
                },
            );
            return Ok(());
        }
        let fetched = self
            .ledger
            .download_once(local, async {
                (self.log)(&format!("fetching {} ({size} bytes)", e.path));
                match self.files.download(e.clone(), PathBuf::from(local)).await {
                    Ok(()) => {
                        self.place(local, local);
                        Ok(())
                    }
                    Err(FileError::InUse(why, part)) => {
                        self.put_beside(local, &aside, &part, remote_mtime, &why)
                    }
                    Err(FileError::Failed(m)) => Err(m),
                }
            })
            .await;
        fetched?;
        if let Some(now) = filetime::stat(Path::new(&self.actual(local))) {
            self.ledger.set(
                local,
                Synced {
                    size: now.size,
                    local_mtime_ms: now.mtime_ms,
                    remote_mtime_ms: remote_mtime,
                },
            );
        }
        Ok(())
    }

    /// The engine holds the old version open: the new one goes beside it, and frames name that.
    fn put_beside(
        &self,
        local: &str,
        aside: &str,
        part: &Path,
        remote_mtime: f64,
        why: &str,
    ) -> Result<(), String> {
        if std::fs::rename(part, aside).is_err() {
            let _ = std::fs::remove_file(part);
            return Err(format!(
                "{why}, and the new version could not be put beside it"
            ));
        }
        filetime::set_mtime_ms(Path::new(aside), remote_mtime);
        (self.log)(&format!("{local} is held open; its new version is {aside}"));
        self.place(local, aside);
        Ok(())
    }

    async fn send_up(&self, named: &str) {
        let Some((remote, root)) = self.to_remote(named) else {
            return;
        };
        if !root.writable {
            return;
        }
        let Some(have) = filetime::stat(Path::new(named)).filter(|h| h.is_file) else {
            return;
        };
        // The ledger is keyed by the canonical path, which a version placed beside a held file stands for.
        let local = self
            .ledger
            .canonical_of(named)
            .unwrap_or_else(|| named.to_string());
        if self
            .ledger
            .get(&local)
            .is_some_and(|k| k.size == have.size && filetime::same(k.local_mtime_ms, have.mtime_ms))
        {
            return;
        }
        (self.log)(&format!("sending {remote} ({} bytes)", have.size));
        match self.files.upload(PathBuf::from(named), remote).await {
            Ok(back) => self.ledger.set(
                &local,
                Synced {
                    size: have.size,
                    local_mtime_ms: have.mtime_ms,
                    remote_mtime_ms: back.mtime_ms.unwrap_or(0.0),
                },
            ),
            Err(err) => (self.log)(&format!("could not send {local}: {err}")),
        }
    }
}
