//! job.rs — the registry of cuts and how one is asked for: keyed by its WORK, started on a thread of its own, and
//! POLLED by whoever asked, once a second, which is how a second request for the same cut joins the first. Jobs
//! past the machine's cap wait `queued`; an hour from its START ends a job whatever it is doing.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::audio::{INSTRUMENTAL, STEMS};
use super::budget::Counted;
use super::cut;
use super::onnx::OnnxSession;
use super::slots::JobSlots;

const TIMEOUT: Duration = Duration::from_secs(3600);
const KEEP: Duration = Duration::from_secs(300);
const POLL: Duration = Duration::from_secs(1);
const MAX_LINES: usize = 50;

pub struct Request {
    pub user: String,
    /// The mix, a local path.
    pub source: String,
    /// The .onnx, a local path.
    pub model: String,
    /// Where the stems land.
    pub out_dir: String,
    /// What to write; the four stems when empty.
    pub outputs: Vec<String>,
    /// Execution providers to try, in order; the CPU when empty.
    pub providers: Vec<String>,
}

struct State {
    json: Value,
    at: Instant,
    session: Option<Arc<OnnxSession>>,
}

/// One cut: its state as the web protocol's `SeparateJob`, and what wakes a waiter when it ends.
pub(super) struct Job {
    state: Mutex<State>,
    ended: Condvar,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Job {
    fn new(id: &str, source: &str) -> Job {
        let json = json!({
            "id": id, "source": source, "done": false, "failed": false, "error": null,
            "phase": "starting", "percent": null, "lines": [], "files": [],
        });
        Job {
            state: Mutex::new(State {
                json,
                at: Instant::now(),
                session: None,
            }),
            ended: Condvar::new(),
        }
    }

    fn done_in(st: &State) -> bool {
        st.json["done"] == true
    }

    pub(super) fn is_done(&self) -> bool {
        Self::done_in(&lock(&self.state))
    }

    /// Progress, as the worker has news: a phase, a percent (None: no measurable end), and maybe a line.
    pub(super) fn say(&self, phase: &str, percent: Option<u32>, line: Option<String>) {
        let mut st = lock(&self.state);
        if Self::done_in(&st) {
            return;
        }
        st.json["phase"] = phase.into();
        st.json["percent"] = percent.map_or(Value::Null, Into::into);
        if let (Some(line), Some(lines)) = (line, st.json["lines"].as_array_mut()) {
            lines.push(line.into());
            if lines.len() > MAX_LINES {
                lines.remove(0);
            }
        }
    }

    pub(super) fn finish(&self, patch: Value) {
        let mut st = lock(&self.state);
        if Self::done_in(&st) {
            return;
        }
        if let Some(fields) = patch.as_object() {
            fields
                .iter()
                .for_each(|(k, v)| st.json[k.as_str()] = v.clone());
        }
        st.json["done"] = true.into();
        st.at = Instant::now();
        self.ended.notify_all();
    }

    fn fail(&self, error: String) {
        self.finish(json!({"failed": true, "error": error, "phase": "failed"}));
    }

    pub(super) fn set_session(&self, session: Arc<OnnxSession>) {
        lock(&self.state).session = Some(session);
    }

    fn snapshot(&self) -> Value {
        lock(&self.state).json.clone()
    }

    /// Waits up to `d` for the job to end; whether it has.
    fn wait_done(&self, d: Duration) -> bool {
        let st = lock(&self.state);
        let (st, _) = self
            .ended
            .wait_timeout_while(st, d, |st| !Self::done_in(st))
            .unwrap_or_else(|e| e.into_inner());
        Self::done_in(&st)
    }

    fn terminate_session(&self) {
        let session = lock(&self.state).session.clone();
        if let Some(s) = session {
            s.terminate();
        }
    }
}

fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}

/// A provider that lost its device once, for this process's lifetime: the next song starts on the CPU.
static LOST: Mutex<String> = Mutex::new(String::new());

pub(super) fn mark_lost(provider: &str) {
    *lock(&LOST) = provider.to_string();
}

/// How many cuts run at once on this machine: `$STUDIO_SEPARATIONS_AT_ONCE`, 1 when unset or not a count.
fn separations_at_once() -> usize {
    std::env::var("STUDIO_SEPARATIONS_AT_ONCE")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1)
}

fn slots() -> &'static JobSlots {
    static SLOTS: OnceLock<JobSlots> = OnceLock::new();
    SLOTS.get_or_init(|| JobSlots::new(separations_at_once()))
}

/// Keyed by the WORK (who, which file, which model, which outputs), never by a workspace.
fn job_id(r: &Request, outputs: &[String]) -> String {
    let key = format!(
        "{}\0{}\0{}\0{}",
        r.user,
        r.source,
        r.model,
        outputs.join(",")
    );
    Sha256::digest(key.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `$STUDIO_ONNX_PROVIDERS` (names split on spaces and commas) over the request's, the CPU when none.
fn providers_for(r: &Request) -> Vec<String> {
    let env = std::env::var("STUDIO_ONNX_PROVIDERS").unwrap_or_default();
    let mut providers: Vec<String> = if !env.trim().is_empty() {
        env.split([' ', ',', '\t', '\n'])
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    } else if lock(&LOST).is_empty() {
        r.providers.clone()
    } else {
        vec!["cpu".into()]
    };
    if providers.is_empty() {
        providers.push("cpu".into());
    }
    providers
}

/// A cut's worker: wait for a slot, start the hour, separate, and say how it ended.
fn work(
    job: Arc<Job>,
    r: Request,
    outputs: Vec<String>,
    providers: Vec<String>,
    _counted: Counted,
) {
    let held = slots().hold(|ahead| {
        let line = if ahead > 0 {
            format!("queued behind {ahead} other cut(s)")
        } else {
            "queued: next, once a running cut ends".to_string()
        };
        job.say("queued", None, Some(line));
    });
    // The hour counts from here: a set list's last song may wait longer than that in the queue.
    let timer = job.clone();
    let source = r.source.clone();
    std::thread::spawn(move || {
        if !timer.wait_done(TIMEOUT) {
            timer.terminate_session();
            timer.fail(format!("separation of {source} exceeded 60 minutes"));
        }
    });
    let ran = catch_unwind(AssertUnwindSafe(|| {
        cut::separate(&job, &r, &outputs, &providers)
    }));
    match ran {
        Ok(Ok(())) => {}
        Ok(Err(e)) => job.fail(e),
        Err(_) => job.fail("the cut stopped unexpectedly".into()),
    }
    drop(held);
}

/// Run (or join) the separation `r` describes, calling `report` with the job (web-protocol's `SeparateJob`) on every change, the last with `done`. Blocks.
pub fn run(r: Request, mut report: impl FnMut(&Value)) -> Result<(), String> {
    let mut outputs = r.outputs.clone();
    if outputs.is_empty() {
        outputs = STEMS.iter().map(|s| s.to_string()).collect();
    }
    if let Some(bad) = outputs
        .iter()
        .find(|o| *o != INSTRUMENTAL && !STEMS.contains(&o.as_str()))
    {
        return Err(format!("'{bad}' is not a separation output"));
    }
    let id = job_id(&r, &outputs);
    let job = {
        let mut all = lock(jobs());
        let cutoff = Instant::now().checked_sub(KEEP);
        all.retain(|_, j| {
            let st = lock(&j.state);
            !(Job::done_in(&st) && cutoff.is_some_and(|c| st.at < c))
        });
        let running = all.get(&id).filter(|j| !j.is_done()).cloned();
        match running {
            Some(job) => job,
            None => {
                let job = Arc::new(Job::new(&id, &r.source));
                all.insert(id, job.clone());
                let providers = providers_for(&r);
                let (worker, counted) = (job.clone(), Counted::new());
                std::thread::spawn(move || work(worker, r, outputs, providers, counted));
                job
            }
        }
    };
    let mut last = String::new();
    loop {
        let snapshot = job.snapshot();
        let said = snapshot.to_string();
        if said != last {
            report(&snapshot);
        }
        last = said;
        if snapshot["done"] == true {
            return Ok(());
        }
        job.wait_done(POLL);
    }
}

/// Ends every cut in progress: the supervisor is going, and a provider that hangs must not hold it.
pub fn terminate_all() {
    let all: Vec<Arc<Job>> = lock(jobs()).values().cloned().collect();
    for job in all {
        if !job.is_done() {
            job.terminate_session();
            job.fail("the supervisor stopped".into());
        }
    }
}
