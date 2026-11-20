//! models.rs — the model downloads' registry as a machine (2440, 2442): jobs by id and by destination, kept after they finish
//! on a clock the caller turns. It starts nothing and fetches nothing; `start` hands back what the I/O side must fetch.
//! What a reader gets wrong: the job's sentences are the TypeScript's, word for word, and `received` counts what was written.

use std::collections::HashMap;
use std::time::Instant;

use serde_json::{Value, json};

use super::KEEP;

/// A download a poll can read.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub id: String,
    /// The model's file name, as the module authored it.
    pub file: String,
    pub url: String,
    pub received: u64,
    /// The size the server announced, none when it did not or the body is encoded.
    pub total: Option<u64>,
    pub done: bool,
    pub error: Option<String>,
}

impl Job {
    /// The route's JSON, keys in the TypeScript's order.
    pub fn to_json(&self) -> Value {
        json!({ "id": self.id, "file": self.file, "url": self.url, "received": self.received, "total": self.total,
                "done": self.done, "error": self.error })
    }
}

/// What the I/O side must fetch for a job `start` made: the url into `dest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub id: String,
    pub url: String,
    pub dest: String,
}

/// What happened to a fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchEvent {
    /// The response came: the size to hold the body to, if one is announced and the body is not encoded.
    Headers { total: Option<u64> },
    /// This many bytes were written.
    Bytes(u64),
    /// The part was renamed over the destination.
    Renamed,
    /// The fetch failed with this reason (the job's sentence is made here).
    Failed(String),
}

struct Entry {
    job: Job,
    dest: Option<String>,
    finished: Option<Instant>,
}

pub struct Models {
    jobs: HashMap<String, Entry>,
    by_dest: HashMap<String, String>,
    mint: Box<dyn FnMut() -> [u8; 16] + Send>,
}

/// A version 4 UUID from 16 random bytes, as `randomUUID` spells one.
pub fn uuid_v4(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        h[0..4].concat(),
        h[4..6].concat(),
        h[6..8].concat(),
        h[8..10].concat(),
        h[10..16].concat()
    )
}

/// `/^https?:\/\//i`.
fn is_http(url: &str) -> bool {
    let b = url.as_bytes();
    let lower = |n: usize| b.get(..n).map(|p| p.to_ascii_lowercase());
    lower(7).is_some_and(|p| p == b"http://") || lower(8).is_some_and(|p| p == b"https://")
}

impl Models {
    /// A registry whose job ids are made from `mint`'s random bytes.
    pub fn new(mint: Box<dyn FnMut() -> [u8; 16] + Send>) -> Models {
        Models {
            jobs: HashMap::new(),
            by_dest: HashMap::new(),
            mint,
        }
    }

    /// Start `file` from `url` into `dest` (none for a name the cache refuses, `exists` when it is already there), or answer
    /// the job already fetching that destination. A job that needs no fetch is done at once.
    pub fn start(
        &mut self,
        file: &str,
        url: &str,
        dest: Option<&str>,
        exists: bool,
        now: Instant,
    ) -> (Job, Option<Fetch>) {
        self.sweep(now);
        if let Some(running) = dest
            .and_then(|d| self.by_dest.get(d))
            .and_then(|id| self.jobs.get(id))
            && !running.job.done
        {
            return (running.job.clone(), None);
        }
        let id = uuid_v4((self.mint)());
        let mut job = Job {
            id: id.clone(),
            file: file.to_string(),
            url: url.to_string(),
            received: 0,
            total: None,
            done: false,
            error: None,
        };
        let refusal = if dest.is_none() {
            Some(Some(format!(
                "'{file}' is not a name a model can have in the cache"
            )))
        } else if !is_http(url) {
            Some(Some(format!("'{url}' is not an http(s) URL")))
        } else {
            exists.then_some(None)
        };
        let fetch = match (&refusal, dest) {
            (None, Some(d)) => Some(Fetch {
                id: id.clone(),
                url: url.to_string(),
                dest: d.to_string(),
            }),
            _ => None,
        };
        let mut entry = Entry {
            job: job.clone(),
            dest: dest.map(str::to_string),
            finished: None,
        };
        if let Some(error) = refusal {
            Self::finish(&mut entry, error, now);
            job = entry.job.clone();
        } else if let Some(d) = dest {
            self.by_dest.insert(d.to_string(), id.clone());
        }
        self.jobs.insert(id, entry);
        (job, fetch)
    }

    fn finish(e: &mut Entry, error: Option<String>, now: Instant) {
        e.job.done = true;
        e.job.error = error;
        e.finished = Some(now);
    }

    /// Tell the registry what happened to a fetch; an event for a finished or unknown job is ignored.
    pub fn on(&mut self, id: &str, event: FetchEvent, now: Instant) {
        self.sweep(now);
        let Some(e) = self.jobs.get_mut(id).filter(|e| !e.job.done) else {
            return;
        };
        match event {
            FetchEvent::Headers { total } => e.job.total = total,
            FetchEvent::Bytes(n) => e.job.received += n,
            FetchEvent::Renamed => Self::finish(e, None, now),
            FetchEvent::Failed(why) => {
                let sentence = format!("could not download {}: {why}", e.job.file);
                Self::finish(e, Some(sentence), now);
            }
        }
    }

    /// Whether the body that ended is the size announced; a short one is a failed download, not a small model (2442).
    pub fn complete(&self, id: &str) -> Result<(), String> {
        match self.jobs.get(id).map(|e| &e.job) {
            Some(j) if j.total.is_some_and(|t| t != j.received) => Err(format!(
                "the download stopped at {} of {} bytes",
                j.received,
                j.total.unwrap_or_default()
            )),
            _ => Ok(()),
        }
    }

    /// How many jobs have not finished: a download in progress is state the host must not reap.
    pub fn running(&self) -> usize {
        self.jobs.values().filter(|e| e.finished.is_none()).count()
    }

    /// The job, or none once it finished `KEEP` ago or never was.
    pub fn job(&self, id: &str, now: Instant) -> Option<Job> {
        let e = self.jobs.get(id)?;
        e.finished
            .is_none_or(|f| now.saturating_duration_since(f) < KEEP)
            .then(|| e.job.clone())
    }

    /// Forget the jobs finished `KEEP` ago.
    fn sweep(&mut self, now: Instant) {
        let gone: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, e)| {
                e.finished
                    .is_some_and(|f| now.saturating_duration_since(f) >= KEEP)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            if let Some(e) = self.jobs.remove(&id)
                && let Some(d) = e.dest.filter(|d| self.by_dest.get(d) == Some(&id))
            {
                self.by_dest.remove(&d);
            }
        }
    }

    /// How many jobs are held, for a bound in a test.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}
