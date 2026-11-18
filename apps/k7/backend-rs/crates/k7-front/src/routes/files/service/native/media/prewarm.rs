//! prewarm.rs — a workspace's prewarm of the active project's waveforms and its status, per workspace beside the account's
//! envelopes (2430 to 2432). What a reader gets wrong: a cancel stops only decodes no request waits on, and a newer run's
//! generation retires an older one, whose tail then neither counts nor sets the status.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::confine::{Disk, confine_to};
use k7_file_service::media::WARM_DECODES;
use serde_json::{Value, json};
use tokio::task::JoinSet;

use super::Counts;
use super::envelopes::Stale;
use super::waveform::{Locator, Waveforms};

/// How long a cancel waits for the decodes it stopped to let go of their files (2431).
const RELEASE: Duration = Duration::from_secs(2);

/// `{warming, done, total}`, the shape the page polls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub warming: bool,
    pub done: usize,
    pub total: usize,
}

impl Status {
    pub fn to_json(self) -> Value {
        json!({ "warming": self.warming, "done": self.done, "total": self.total })
    }
}

#[derive(Default)]
struct Ws {
    run: u64,
    status: Status,
}

/// Every workspace's generation and status, locked only to read or write them.
#[derive(Clone, Default)]
pub struct Prewarms {
    map: Arc<Mutex<HashMap<String, Ws>>>,
}

impl Prewarms {
    fn with<R>(&self, ws: &str, f: impl FnOnce(&mut Ws) -> R) -> R {
        let mut m = self.map.lock().unwrap_or_else(|p| p.into_inner());
        f(m.entry(ws.to_string()).or_default())
    }

    /// Whether any workspace's prewarm is still warming.
    pub fn warming(&self) -> bool {
        let m = self.map.lock().unwrap_or_else(|p| p.into_inner());
        m.values().any(|w| w.status.warming)
    }

    pub fn status(&self, ws: &str) -> Status {
        self.with(ws, |w| w.status)
    }

    /// The run is still the newest, and `f` applied to its status when it is.
    fn current(&self, ws: &str, run: u64, f: impl FnOnce(&mut Status)) -> bool {
        self.with(ws, |w| {
            let live = w.run == run;
            if live {
                f(&mut w.status);
            }
            live
        })
    }

    /// Retire the running prewarm and stop the decodes no request waits on, answering once they let go (2431).
    pub async fn cancel(&self, ws: &str, waves: &Waveforms) -> Status {
        self.with(ws, |w| {
            w.run += 1;
            w.status = Status::default();
        });
        if waves.envelopes.cancel_prewarm(ws) > 0 {
            let until = tokio::time::Instant::now() + RELEASE;
            while waves.envelopes.stopping() > 0 && tokio::time::Instant::now() < until {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        self.status(ws)
    }

    /// The next generation of `ws`'s prewarm, taken now: a cancel told after it retires it, whenever its task first runs.
    pub fn begin(&self, ws: &str) -> u64 {
        self.with(ws, |w| {
            w.run += 1;
            w.run
        })
    }

    /// Start a prewarm of the active project: the stale list is made before this returns, as the TypeScript's first
    /// synchronous half is, so the answer already says how many; the decodes run on (2430).
    pub async fn start(
        &self,
        ws: &str,
        waves: Waveforms,
        locator: Arc<dyn Locator>,
        counts: Arc<Counts>,
    ) -> Status {
        let run = self.begin(ws);
        self.run(ws, run, waves, locator, counts).await
    }

    /// The prewarm of generation `run`, taken by `begin`.
    pub async fn run(
        &self,
        ws: &str,
        run: u64,
        waves: Waveforms,
        locator: Arc<dyn Locator>,
        counts: Arc<Counts>,
    ) -> Status {
        Counts::bump(&counts.prewarms);
        let Some(plan) = plan(&waves, locator).await else {
            self.current(ws, run, |s| *s = Status::default());
            return self.status(ws);
        };
        let total = plan.stale.sources.len() + plan.stale.unreadable.len();
        // An unreadable sidecar is counted failed without a decode (2415).
        let done = plan.stale.unreadable.len();
        let set = Status {
            warming: total > 0,
            done,
            total,
        };
        // A cancel that landed during the plan retired this run: nothing starts.
        if !self.current(ws, run, |s| *s = set) {
            return self.status(ws);
        }
        let (me, ws2) = (self.clone(), ws.to_string());
        tokio::spawn(async move { me.finish(&ws2, run, &waves, plan, (done, total)).await });
        set
    }

    /// Phase one, the decodes two at a time, then phase two.
    async fn finish(
        &self,
        ws: &str,
        run: u64,
        waves: &Waveforms,
        plan: Plan,
        (mut done, total): (usize, usize),
    ) {
        let mut todo = plan.stale.sources.iter().cloned();
        let mut set = JoinSet::new();
        loop {
            while set.len() < WARM_DECODES {
                if !self.current(ws, run, |_| ()) {
                    return;
                }
                let Some(src) = todo.next() else { break };
                let w = waves.clone();
                let wsn = ws.to_string();
                set.spawn(async move { w.envelopes.warm(&src, &wsn).await });
            }
            if set.join_next().await.is_none() {
                break;
            }
            // A file that fails is counted and skipped; the on-demand path surfaces its error (2432).
            done += 1;
            if !self.current(ws, run, |s| s.done = done) {
                return;
            }
        }
        // Phase two warms the envelopes' memo; the sliced memo is filled by a request, whose signature it knows (2430).
        for src in &plan.all {
            if !self.current(ws, run, |_| ()) {
                return;
            }
            let _ = waves.envelopes.ensure(src, None).await;
        }
        self.current(ws, run, |s| {
            *s = Status {
                warming: false,
                done: total,
                total,
            }
        });
    }
}

/// What a prewarm works through: the project's confined sources and those whose sidecar is stale.
struct Plan {
    all: Vec<PathBuf>,
    stale: Stale,
}

async fn plan(waves: &Waveforms, locator: Arc<dyn Locator>) -> Option<Plan> {
    let view = locator.project().await.ok()?;
    let env = waves.envelopes.clone();
    // Every stat of the project's clips runs on the blocking pool, never on a runtime thread.
    tokio::task::spawn_blocking(move || {
        let roots: Vec<&str> = view.roots.iter().map(String::as_str).collect();
        let all: Vec<PathBuf> = view
            .clips
            .iter()
            .filter_map(|c| confine_to(&roots, &view.project_path(&c.source), &Disk))
            .map(|p| super::waveform::real_path(std::path::Path::new(&p)))
            .collect();
        let stale = env.stale(&all);
        Plan { all, stale }
    })
    .await
    .ok()
}
