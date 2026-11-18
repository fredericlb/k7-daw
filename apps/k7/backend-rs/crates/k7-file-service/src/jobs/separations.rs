//! separations.rs — the registry of `remoteSeparation.ts` and `separationId` of `separateJobs.ts`, with the clock a caller gives (2330).
//! What a reader gets wrong: a request that outlives its record (the same work started again once done) must not write
//! into the new one, so each `start` hands back a generation the later calls name.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A finished job is kept this long for its last poll.
pub const KEEP: Duration = Duration::from_secs(5 * 60);

/// What a job says when no machine is bound.
pub const NO_ENGINE: &str = "no engine to cut on — open this project on an engine first";

/// Keyed by the WORK, not by a workspace: who, which file, which model, which outputs.
pub fn separation_id(user: &str, source: &str, model: &str, outputs: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(format!("{user}\0{source}\0{model}\0{}", outputs.join(",")).as_bytes());
    h.finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

struct Held {
    job: Map<String, Value>,
    at: Instant,
    generation: u64,
}

/// What `start` decided: the record to answer, and whether the caller must send the machine its request.
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub job: Map<String, Value>,
    pub generation: u64,
    pub send: bool,
}

#[derive(Default)]
pub struct Registry {
    jobs: HashMap<String, Held>,
    next: u64,
}

fn blank(id: &str, source: &str) -> Map<String, Value> {
    let Value::Object(m) = json!({
        "id": id, "source": source, "done": false, "failed": false, "error": null,
        "phase": "sending", "percent": null, "lines": [], "files": [],
    }) else {
        unreachable!()
    };
    m
}

/// `{...job, done: true, failed: true, error, phase: "failed"}`.
fn failed(job: &mut Map<String, Value>, error: &str) {
    job.insert("done".into(), Value::Bool(true));
    job.insert("failed".into(), Value::Bool(true));
    job.insert("error".into(), Value::String(error.to_owned()));
    job.insert("phase".into(), Value::String("failed".into()));
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Starts the work, or joins the one running; `bound` is whether a machine's jobs path is bound now.
    pub fn start(&mut self, now: Instant, id: &str, source: &str, bound: bool) -> Started {
        let cutoff = now.checked_sub(KEEP);
        self.jobs.retain(|_, h| {
            !(h.job.get("done") == Some(&Value::Bool(true)) && cutoff.is_some_and(|c| h.at < c))
        });
        if let Some(h) = self.jobs.get(id)
            && h.job.get("done") != Some(&Value::Bool(true))
        {
            return Started {
                job: h.job.clone(),
                generation: h.generation,
                send: false,
            };
        }
        self.next += 1;
        let mut held = Held {
            job: blank(id, source),
            at: now,
            generation: self.next,
        };
        if !bound {
            failed(&mut held.job, NO_ENGINE);
        }
        let started = Started {
            job: held.job.clone(),
            generation: held.generation,
            send: bound,
        };
        self.jobs.insert(id.to_owned(), held);
        started
    }

    /// The machine's record, taken with this side's id; a request of an older generation says nothing.
    pub fn progress(
        &mut self,
        now: Instant,
        id: &str,
        generation: u64,
        record: Map<String, Value>,
    ) {
        if let Some(h) = self.jobs.get_mut(id).filter(|h| h.generation == generation) {
            h.job = record;
            h.job.insert("id".into(), Value::String(id.to_owned()));
            h.at = now;
        }
    }

    /// The request ended in an error: `from_machine` is whether the machine itself said it (a `JobError`).
    pub fn fail(
        &mut self,
        now: Instant,
        id: &str,
        generation: u64,
        from_machine: bool,
        message: &str,
    ) {
        if let Some(h) = self.jobs.get_mut(id).filter(|h| h.generation == generation) {
            let said = if from_machine {
                message.to_owned()
            } else {
                format!("the engine's machine: {message}")
            };
            failed(&mut h.job, &said);
            h.at = now;
        }
    }

    pub fn job(&self, id: &str) -> Option<Map<String, Value>> {
        self.jobs.get(id).map(|h| h.job.clone())
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}
