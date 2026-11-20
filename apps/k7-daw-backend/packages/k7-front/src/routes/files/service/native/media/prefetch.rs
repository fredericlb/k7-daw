//! prefetch.rs — the desk records the open projects name, put onto the bound machine ahead of time (2460, 2461): one `prefetch` job
//! over the jobs path, on a timer, again only when the list or the machine moved. What a reader gets wrong: stopping aborts
//! the task, which drops the job's socket mid-request; nothing else cancels a prefetch, as in the TypeScript.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::media::names::prefetch_signature;
use serde_json::{Map, json};
use tokio::task::JoinHandle;

use super::Counts;
use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, JobChannel, of_type, request_job};
use crate::routes::files::service::native::engine::EngineLink;
use crate::routes::files::service::native::engine::jobs::WsChannel;

/// The first ask, delayed so a cold start does not compete with the binding it needs.
pub const FIRST: Duration = Duration::from_secs(5);
/// How often the open projects are asked what they name.
pub const EVERY: Duration = Duration::from_secs(60);

/// What a prefetch reads of the account: where the bound machine takes jobs, and what the open projects name.
pub trait Source: Send + Sync + 'static {
    fn jobs_url(&self) -> Option<String>;
    /// Every record the open projects' desks name, and its stems, sorted; it reads the disk, so it is called off the runtime.
    fn named_files(&self) -> Vec<String>;
    fn say(&self, line: &str);
}

/// The engine link as a source: the open projects are the ones the account's markers name, served or not.
pub struct LinkSource(pub EngineLink);

impl Source for LinkSource {
    fn jobs_url(&self) -> Option<String> {
        self.0.jobs_url()
    }

    fn named_files(&self) -> Vec<String> {
        let inner = &self.0.0;
        let mut files: Vec<String> = inner
            .host
            .open_projects(&inner.account.files_root)
            .iter()
            .flat_map(|(root, project)| inner.host.named_files(root, project))
            .collect();
        files.sort();
        files.dedup();
        files
    }

    fn say(&self, line: &str) {
        self.0.0.tells.say(line);
    }
}

/// The account's prefetch timer: one task, stopped by `stop` and by the drop.
#[derive(Default)]
pub struct Prefetch {
    task: Mutex<Option<JoinHandle<()>>>,
    asking: Arc<AtomicBool>,
}

impl Prefetch {
    /// Whether a job is out right now.
    pub fn asking(&self) -> bool {
        self.asking.load(Ordering::Acquire)
    }

    /// Whether the timer runs.
    pub fn started(&self) -> bool {
        self.task
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|t| !t.is_finished())
    }

    /// Start the timer once; it asks after `first`, then every `every`.
    pub fn start(
        &self,
        src: Arc<dyn Source>,
        counts: Arc<Counts>,
        moved: Arc<dyn Fn() + Send + Sync>,
        first: Duration,
        every: Duration,
    ) {
        let mut slot = self.task.lock().unwrap_or_else(|p| p.into_inner());
        if slot.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let asking = self.asking.clone();
        *slot = Some(tokio::spawn(async move {
            let (mut settled, mut last_error, mut last_state) =
                (String::new(), String::new(), String::new());
            tokio::time::sleep(first).await;
            loop {
                once(
                    &src,
                    &counts,
                    &moved,
                    &asking,
                    &mut settled,
                    &mut last_error,
                    &mut last_state,
                )
                .await;
                tokio::time::sleep(every).await;
            }
        }));
    }

    /// Stop the timer and drop the job in flight.
    pub fn stop(&self) {
        if let Some(t) = self.task.lock().unwrap_or_else(|p| p.into_inner()).take() {
            t.abort();
        }
        self.asking.store(false, Ordering::Release);
    }
}

impl Drop for Prefetch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One pass: send the named files to the bound machine if the list or the machine moved. Never fails.
async fn once(
    src: &Arc<dyn Source>,
    counts: &Counts,
    moved: &Arc<dyn Fn() + Send + Sync>,
    asking: &AtomicBool,
    settled: &mut String,
    last_error: &mut String,
    last_state: &mut String,
) {
    let url = src.jobs_url();
    let s = src.clone();
    let Ok(files) = tokio::task::spawn_blocking(move || s.named_files()).await else {
        return;
    };
    let state = format!("machine={} files={}", url.is_some(), files.len());
    if state != *last_state {
        src.say(&format!("   record prefetch: {state}"));
        *last_state = state;
    }
    let Some(url) = url else { return };
    if files.is_empty() {
        return;
    }
    let signature = prefetch_signature(&url, &files);
    if signature == *settled {
        return;
    }
    Counts::bump(&counts.prefetches);
    moved();
    asking.store(true, Ordering::Release);
    let done = ask(&url, files).await;
    asking.store(false, Ordering::Release);
    match done {
        Ok(()) => {
            *settled = signature;
            last_error.clear();
        }
        Err(message) => {
            if message != *last_error {
                src.say(&format!("   record prefetch: {message}"));
            }
            *last_error = message;
        }
    }
}

/// The `prefetch` job, settled by `prefetched`.
async fn ask(url: &str, files: Vec<String>) -> Result<(), String> {
    let mut chan = WsChannel::open(url).await?;
    let body: Map<String, serde_json::Value> = json!({ "type": "prefetch", "files": files })
        .as_object()
        .cloned()
        .unwrap_or_default();
    let done = request_job(
        &mut chan,
        body,
        of_type("prefetched"),
        |_| {},
        JOB_TOTAL,
        JOB_IDLE,
    )
    .await;
    chan.close();
    done.map(|_| ()).map_err(|e| e.0)
}
