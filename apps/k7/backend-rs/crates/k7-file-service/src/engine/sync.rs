//! The copy to the engine's machine (2223): every file a document names is on the bound machine before the document is
//! sent, copied by one `sync` job whose progress the poll reports. A failed copy is held until the person retries.
//! What a reader gets wrong: `settled` is only ever set by a job that answered; a path outside the roots is never stated (2224).

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

/// A file as the machine settles it: its size and its mtime in rounded milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileKey {
    pub size: u64,
    pub mtime_ms: i64,
}

/// `EngineSyncState`: what the poll shows of a copy in flight, or of the last one that failed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncState {
    pub files: u64,
    pub files_done: u64,
    pub bytes: u64,
    pub bytes_done: u64,
    pub failed: Option<u64>,
    pub failures: Option<Vec<String>>,
    pub error: Option<String>,
}

impl SyncState {
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("files".into(), json!(self.files));
        o.insert("filesDone".into(), json!(self.files_done));
        o.insert("bytes".into(), json!(self.bytes));
        o.insert("bytesDone".into(), json!(self.bytes_done));
        if let Some(n) = self.failed {
            o.insert("failed".into(), json!(n));
        }
        if let Some(f) = &self.failures {
            o.insert("failures".into(), json!(f));
        }
        if let Some(e) = &self.error {
            o.insert("error".into(), json!(e));
        }
        Value::Object(o)
    }
}

/// What the copy asks the front to do.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncEffect {
    /// Send this document now (as a `project` frame the front stamps and queues).
    Send(Value),
    /// Run one `sync` job over `url` for these files.
    Copy {
        url: String,
        files: Vec<String>,
    },
    Log(String),
}

/// How a job ended.
#[derive(Debug, Clone)]
pub enum Reply {
    /// The `synced` frame.
    Synced(Value),
    /// The channel failed, or the machine refused: the message, as the jobs client words it.
    Failed(String),
}

/// What the machine is asked about files: the bound jobs path, the file system's answer, the roots a path must stay in.
pub struct Ctx<'a> {
    pub url: Option<&'a str>,
    /// Answered from a snapshot taken BEFORE the machine runs: the front states the named paths on the blocking pool,
    /// outside every lock, then calls the machine, which must not touch the disk.
    pub stat: &'a dyn Fn(&str) -> Option<FileKey>,
    pub roots: Option<&'a [String]>,
    /// The host spells its paths the Windows way: the front's own `cfg!(windows)`, never this crate's.
    pub windows: bool,
}

struct Run {
    doc: Value,
    state: SyncState,
    todo: Vec<(String, FileKey)>,
    url: String,
}

/// The account's copy state: one machine is bound at a time, so what it holds is shared by the sessions.
#[derive(Default)]
pub struct Copies {
    settled: HashMap<String, FileKey>,
    settled_for: Option<String>,
    legacy: Option<String>,
    runs: HashMap<String, Run>,
    failures: HashMap<String, SyncState>,
}

/// An inline source is not a path, and a path holds no line break; `windows` is the host's spelling.
fn is_path(s: &str, windows: bool) -> bool {
    if s.contains('\n') {
        return false;
    }
    let b = s.as_bytes();
    s.starts_with('/')
        || (windows
            && (s.starts_with('\\')
                || (b.len() >= 3
                    && b[0].is_ascii_alphabetic()
                    && b[1] == b':'
                    && (b[2] == b'/' || b[2] == b'\\'))))
}

/// `collect`: every absolute path a document names, in the order it names them, once each.
pub fn collect(v: &Value, into: &mut Vec<String>, windows: bool) {
    let mut seen: HashSet<String> = into.iter().cloned().collect();
    walk(v, into, &mut seen, windows);
}

fn walk(v: &Value, into: &mut Vec<String>, seen: &mut HashSet<String>, windows: bool) {
    match v {
        Value::String(s) => {
            if is_path(s, windows) && seen.insert(s.clone()) {
                into.push(s.clone());
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, into, seen, windows)),
        Value::Object(o) => o.values().for_each(|x| walk(x, into, seen, windows)),
        _ => {}
    }
}

/// `Number(x) || 0`: a count the machine sent, however it was spelt.
fn count(v: Option<&Value>) -> u64 {
    let n = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    };
    if n.is_finite() && n > 0.0 {
        n as u64
    } else {
        0
    }
}

/// A path as the Windows host compares it: `\` read as `/`, and a drive letter in capitals.
fn spelt(p: &str, windows: bool) -> String {
    if !windows {
        return p.to_string();
    }
    let mut s = p.replace('\\', "/");
    if s.as_bytes().get(1) == Some(&b':') && s.as_bytes()[0].is_ascii_alphabetic() {
        s[..1].make_ascii_uppercase();
    }
    s
}

/// 2224: a path is stated only when it lies in one of the roots, with no `..` to climb out by. On a Windows host the
/// separators and the drive's case do not tell two spellings of one path apart.
pub fn within(path: &str, roots: &[String], windows: bool) -> bool {
    let path = spelt(path, windows);
    let sep: &[char] = if windows { &['/'] } else { &['/', '\\'] };
    !path.split(['/', '\\']).any(|s| s == "..")
        && roots.iter().any(|r| {
            let r = spelt(r, windows);
            let r = r.trim_end_matches(sep);
            path == r || (path.starts_with(r) && path[r.len()..].starts_with(sep))
        })
}

impl Copies {
    pub fn new() -> Copies {
        Copies::default()
    }

    /// What the poll shows for `session`: a copy in flight, the last one that failed, or null.
    pub fn state(&self, session: &str) -> Option<&SyncState> {
        self.runs
            .get(session)
            .map(|r| &r.state)
            .or_else(|| self.failures.get(session))
    }

    /// Whether a copy, running or failed, holds `session`'s document back: the engine not holding it is no fault of its own.
    pub fn is_held_back(&self, session: &str) -> bool {
        self.runs.contains_key(session) || self.failures.contains_key(session)
    }

    /// Let the next document try again: what the browser's "Reload project" asks for.
    pub fn forget_failure(&mut self, session: &str) {
        self.failures.remove(session);
    }

    /// Send `doc` once every file it (and `extra`) names is on the bound machine; at once when nothing is missing.
    pub fn ask(
        &mut self,
        session: &str,
        doc: Value,
        extra: Vec<String>,
        ctx: &Ctx,
    ) -> Vec<SyncEffect> {
        let Some(url) = ctx.url else {
            return vec![SyncEffect::Send(doc)];
        };
        if self.legacy.as_deref() == Some(url) {
            return vec![SyncEffect::Send(doc)];
        }
        if self.settled_for.as_deref() != Some(url) {
            self.settled.clear();
            self.settled_for = Some(url.to_string());
        }
        // Fatal until the person asks again: a project missing files would play with silent clips.
        if self.failures.contains_key(session) {
            return vec![];
        }
        if let Some(run) = self.runs.get_mut(session) {
            run.doc = doc;
            return vec![];
        }
        let todo = self.unsettled(&doc, &extra, ctx);
        if todo.is_empty() {
            return vec![SyncEffect::Send(doc)];
        }
        let state = SyncState {
            files: todo.len() as u64,
            bytes: todo.iter().map(|(_, k)| k.size).sum(),
            ..SyncState::default()
        };
        let files = todo.iter().map(|(p, _)| p.clone()).collect();
        self.runs.insert(
            session.to_string(),
            Run {
                doc,
                state,
                todo,
                url: url.to_string(),
            },
        );
        vec![SyncEffect::Copy {
            url: url.to_string(),
            files,
        }]
    }

    /// The files `doc` and `extra` name that the machine is not known to hold as they are here.
    fn unsettled(&self, doc: &Value, extra: &[String], ctx: &Ctx) -> Vec<(String, FileKey)> {
        let mut named: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for p in extra {
            if seen.insert(p.clone()) {
                named.push(p.clone());
            }
        }
        walk(doc, &mut named, &mut seen, ctx.windows);
        let mut out = Vec::new();
        for path in named {
            if ctx.roots.is_some_and(|r| !within(&path, r, ctx.windows)) {
                continue;
            }
            let Some(key) = (ctx.stat)(&path) else {
                continue;
            };
            if self.settled.get(&path) != Some(&key) {
                out.push((path, key));
            }
        }
        out
    }

    /// A `syncing` frame: the machine counts only what it had to copy, which is what the bar is about.
    pub fn progress(&mut self, session: &str, frame: &Value) {
        if let Some(run) = self.runs.get_mut(session) {
            run.state.files = count(frame.get("files"));
            run.state.files_done = count(frame.get("filesDone"));
            run.state.bytes = count(frame.get("bytes"));
            run.state.bytes_done = count(frame.get("bytesDone"));
        }
    }

    /// The document of the copy running for `session`, which the front states the paths of before the copy ends.
    pub fn run_doc(&self, session: &str) -> Option<Value> {
        self.runs.get(session).map(|r| r.doc.clone())
    }

    /// The job ended. A failed copy sends nothing; otherwise the latest document goes through the gate again, with `extra`
    /// asked afresh as the TypeScript asks it at the end.
    pub fn finished(
        &mut self,
        session: &str,
        reply: Reply,
        extra: Vec<String>,
        ctx: &Ctx,
    ) -> Vec<SyncEffect> {
        let Some(run) = self.runs.remove(session) else {
            return vec![];
        };
        let mut fx = vec![];
        match reply {
            Reply::Synced(last) => {
                let failed = count(last.get("failed"));
                let why: Vec<String> = last
                    .get("failures")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                // A file that did not arrive stays unsettled, so the retry asks for it again.
                if failed > 0 {
                    fx.push(SyncEffect::Log(format!(
                        "   \u{2717} {failed} file(s) did not reach the engine's machine: {}",
                        why.join("; ")
                    )));
                    self.failures.insert(
                        session.to_string(),
                        SyncState {
                            failed: Some(failed),
                            failures: Some(why),
                            ..run.state.clone()
                        },
                    );
                } else {
                    // Only the machine the copy was for: a binding that moved meanwhile settled nothing of the new one (F6).
                    if self.settled_for.as_deref() == Some(run.url.as_str()) {
                        for (p, k) in &run.todo {
                            self.settled.insert(p.clone(), *k);
                        }
                    }
                    self.failures.remove(session);
                }
            }
            Reply::Failed(message) => {
                if message.contains("does not do 'sync'") {
                    self.legacy = Some(run.url.clone());
                    self.failures.remove(session);
                } else {
                    fx.push(SyncEffect::Log(format!(
                        "   \u{2717} could not copy the project's files: {message}"
                    )));
                    self.failures.insert(
                        session.to_string(),
                        SyncState {
                            error: Some(message),
                            ..run.state.clone()
                        },
                    );
                }
            }
        }
        if !self.failures.contains_key(session) {
            fx.extend(self.ask(session, run.doc, extra, ctx));
        }
        fx
    }
}
