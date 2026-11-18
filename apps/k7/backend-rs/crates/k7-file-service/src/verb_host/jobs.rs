//! jobs.rs — the table of the jobs the host members start: one poll loop serves them all, and the poll is the heartbeat (3f, 3-B3).
//! What a reader gets wrong: a job is stopped when nobody polls, not when it is slow; `prepare` and `picture` opt out.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};

use super::Error;

/// A job nobody polled for this long is stopped.
pub const HEARTBEAT: Duration = Duration::from_secs(60);
/// How long a finished job's record outlives its end.
pub const KEEP: Duration = Duration::from_secs(600);
/// The most jobs one account runs at once.
pub const MAX_RUNNING: usize = 8;

/// Set when a job is stopped, for the driver to see.
pub type Cancel = Arc<AtomicBool>;

enum State {
    Running { fraction: f64, phase: String },
    Done(Value),
    Failed { error: String, code: Option<String> },
}

struct Job {
    account: String,
    /// False until the driver returns: the place it holds in `MAX_RUNNING` outlives the record's end.
    released: bool,
    beat: bool,
    last_poll: Duration,
    state: State,
    ended: Option<Duration>,
    cancel: Cancel,
}

/// The jobs of every account, by id; an id is 128 random bits and is no capability across accounts.
#[derive(Default)]
pub struct Table {
    jobs: Mutex<HashMap<String, Job>>,
}

static GLOBAL: OnceLock<Table> = OnceLock::new();

fn mint() -> Result<String, Error> {
    let mut b = [0u8; 16];
    // A failed entropy source is a failed start, never a guessable id.
    getrandom::fill(&mut b).map_err(|_| Error::new(503, "no random source to name a job"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

impl Table {
    pub fn global() -> &'static Table {
        GLOBAL.get_or_init(Table::default)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a job; `beat` false lets it run with no one polling. A 503 `busy` past `MAX_RUNNING`.
    pub fn start(
        &self,
        account: &str,
        beat: bool,
        now: Duration,
    ) -> Result<(String, Cancel), Error> {
        let mut jobs = self.lock();
        let running = jobs
            .values()
            .filter(|j| j.account == account && !j.released)
            .count();
        if running >= MAX_RUNNING {
            return Err(Error::coded(
                503,
                "too many jobs are running, ask again",
                "busy",
            ));
        }
        let id = mint()?;
        if jobs.contains_key(&id) {
            return Err(Error::new(503, "a job id collided, ask again"));
        }
        let cancel: Cancel = Arc::new(AtomicBool::new(false));
        jobs.insert(
            id.clone(),
            Job {
                account: account.to_string(),
                released: false,
                beat,
                last_poll: now,
                state: State::Running {
                    fraction: 0.0,
                    phase: String::new(),
                },
                ended: None,
                cancel: cancel.clone(),
            },
        );
        Ok((id, cancel))
    }

    /// What the job reports while it runs; ignored once it has ended.
    pub fn progress(&self, id: &str, fraction: f64, phase: &str) {
        if let Some(Job {
            state:
                State::Running {
                    fraction: f,
                    phase: p,
                },
            ..
        }) = self.lock().get_mut(id)
        {
            *f = fraction.clamp(0.0, 1.0);
            *p = phase.to_string();
        }
    }

    /// Ends the job with its value; false when it had already ended (a stopped job keeps its stop).
    pub fn finish(&self, id: &str, value: Value, now: Duration) -> bool {
        self.end(id, State::Done(value), now)
    }

    pub fn fail(&self, id: &str, error: &str, code: Option<&str>, now: Duration) -> bool {
        let state = State::Failed {
            error: error.to_string(),
            code: code.map(str::to_string),
        };
        self.end(id, state, now)
    }

    fn end(&self, id: &str, state: State, now: Duration) -> bool {
        let mut jobs = self.lock();
        match jobs.get_mut(id) {
            Some(j) if j.ended.is_none() => {
                j.state = state;
                j.ended = Some(now);
                true
            }
            _ => false,
        }
    }

    /// The job's answer to its owner, and the poll that keeps it alive; `None` for another account's, an unknown or a forgotten one.
    pub fn poll(&self, account: &str, id: &str, now: Duration) -> Option<Value> {
        let mut jobs = self.lock();
        let j = jobs.get_mut(id).filter(|j| j.account == account)?;
        if j.ended.is_some_and(|e| now.saturating_sub(e) > KEEP) {
            jobs.remove(id);
            return None;
        }
        j.last_poll = now;
        Some(match &j.state {
            State::Running { fraction, phase } => {
                json!({ "state": "running", "fraction": fraction, "phase": phase })
            }
            State::Done(v) => json!({ "state": "done", "value": v }),
            State::Failed { error, code } => match code {
                Some(c) => json!({ "state": "failed", "error": error, "code": c }),
                None => json!({ "state": "failed", "error": error }),
            },
        })
    }

    /// The driver is done with the job, whatever its end: its place is free and a record it never finished is stopped.
    pub fn release(&self, id: &str, now: Duration) {
        let mut jobs = self.lock();
        if let Some(j) = jobs.get_mut(id) {
            j.released = true;
            if j.ended.is_none() {
                j.state = State::Failed {
                    error: "the job ended without an answer".into(),
                    code: None,
                };
                j.ended = Some(now);
            }
        }
    }

    /// Whether the job ended with its value: the only state a change set may adopt the files of.
    pub fn done(&self, id: &str) -> bool {
        self.lock()
            .get(id)
            .is_some_and(|j| matches!(j.state, State::Done(_)))
    }

    /// Whether the job is `account`'s and ended with its value.
    pub fn finished(&self, account: &str, id: &str) -> bool {
        self.lock()
            .get(id)
            .is_some_and(|j| j.account == account && matches!(j.state, State::Done(_)))
    }

    /// How many jobs of the account have a driver still running.
    pub fn running(&self, account: &str) -> usize {
        self.lock()
            .values()
            .filter(|j| j.account == account && !j.released)
            .count()
    }

    /// Stops the jobs nobody polled for `HEARTBEAT` and forgets the ones ended `KEEP` ago; the stopped ids.
    pub fn tick(&self, now: Duration) -> Vec<String> {
        let mut jobs = self.lock();
        jobs.retain(|_, j| j.ended.is_none_or(|e| now.saturating_sub(e) <= KEEP));
        let mut stopped = Vec::new();
        for (id, j) in jobs.iter_mut() {
            if j.beat && j.ended.is_none() && now.saturating_sub(j.last_poll) > HEARTBEAT {
                j.cancel.store(true, Ordering::Relaxed);
                j.state = State::Failed {
                    error: "stopped: nobody polled the job".into(),
                    code: Some("gone".into()),
                };
                j.ended = Some(now);
                stopped.push(id.clone());
            }
        }
        stopped
    }
}
