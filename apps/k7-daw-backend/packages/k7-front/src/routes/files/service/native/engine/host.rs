//! The link's `Tells` (2240, 2251): a take the engine reports is filed into the project its session serves, and the link's
//! lines are said.
//! What a reader gets wrong: a take is filed by one worker per session, in the order the takes ended.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::wire::Take;
use serde_json::{Value, json};

use super::{EngineLink, Host, Tells, held};

/// One account's bridge: the link's `Tells`.
pub struct Bridge {
    me: std::sync::Weak<Bridge>,
    /// Weak: the link holds the bridge as its `Tells`, so a strong handle here would keep both alive for good.
    link: OnceLock<std::sync::Weak<super::Inner>>,
    host: Arc<dyn Host>,
    rt: tokio::runtime::Handle,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    takes: Mutex<HashMap<String, Value>>,
    filings: Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Filing>>>,
}

impl Bridge {
    /// Must be made inside a runtime: its consumer runs on it.
    pub fn new(host: Arc<dyn Host>, log: Arc<dyn Fn(&str) + Send + Sync>) -> Arc<Bridge> {
        Arc::new_cyclic(|me| Bridge {
            me: me.clone(),
            link: OnceLock::new(),
            host,
            rt: tokio::runtime::Handle::current(),
            log,
            takes: Mutex::new(HashMap::new()),
            filings: Mutex::new(HashMap::new()),
        })
    }

    /// The link this bridge serves, once made (the link needs the bridge as its `Tells`).
    pub fn set_link(&self, link: EngineLink) {
        let _ = self.link.set(Arc::downgrade(&link.0));
    }

    /// The link while its account holds it.
    fn link(&self) -> Option<EngineLink> {
        self.link
            .get()
            .and_then(std::sync::Weak::upgrade)
            .map(EngineLink)
    }

    /// `lastTake` of a session, from the answer of its filing (2242).
    pub fn last_take(&self, session: &str) -> Option<Value> {
        held(&self.takes).get(session).cloned()
    }
}

/// One take waiting its turn: the project its session served when it ended, and the arm held then.
pub(super) struct Filing {
    pub(super) root: String,
    pub(super) project: String,
    pub(super) take: Take,
    pub(super) arm: Option<TakeArm>,
}

impl Bridge {
    /// A take filed into the project the session serves NOW, queued at once on the session's own channel so the takes are filed in the
    /// order they ended (B3, C6): one worker per session awaits each filing before it starts the next.
    fn file_take(&self, session: &str, take: Take) {
        let served = self
            .link()
            .and_then(|l| l.find(session))
            .and_then(|s| s.served());
        let Some(served) = served else {
            (self.log)(&format!(
                "[rec] {}: recorded {}, but session {session} is serving no project here — the clip was NOT added to any track",
                take.track, take.file
            ));
            return;
        };
        // The arm at report time, not at filing time: a disarm told meanwhile must not change where this take goes.
        let arm = self
            .link()
            .and_then(|l| l.find(session))
            .and_then(|s| held(&s.st).take_arm.clone());
        let job = Filing {
            root: served.root,
            project: served.project,
            take,
            arm,
        };
        let mut queues = held(&self.filings);
        let tx = queues.entry(session.to_string()).or_insert_with(|| {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            self.rt.spawn(file_all(
                self.me.clone(),
                self.log.clone(),
                session.to_string(),
                rx,
            ));
            tx
        });
        let _ = tx.send(job);
    }

    /// One filing run to its end on a blocking thread; the project is recompiled and sent once the clip is in.
    async fn file_now(self: &Arc<Self>, session: &str, job: Filing) {
        let (host, root, project) = (self.host.clone(), job.root.clone(), job.project.clone());
        let (take, arm) = (job.take.clone(), job.arm.clone());
        let done = tokio::task::spawn_blocking(move || {
            host.file_take(&root, &project, &take, arm.as_ref())
        })
        .await;
        let track = job.take.track;
        match done {
            Ok(Ok(id)) => {
                (self.log)(&format!("[rec] {track}: added clip {id}"));
                self.host.invalidate(&job.root);
                if let Some(s) = self.link().and_then(|l| l.find(session)) {
                    s.push_project(&super::queue::Served {
                        root: job.root,
                        project: job.project,
                    })
                    .await;
                }
                held(&self.takes)
                    .insert(session.to_string(), json!({ "clip": id, "track": track }));
            }
            Ok(Err(e)) => (self.log)(&format!(
                "[rec] {track}: recorded {} but could not add it to the track: {e}",
                job.take.file
            )),
            Err(e) => (self.log)(&format!(
                "[rec] {track}: recorded {} but the filing failed: {e}",
                job.take.file
            )),
        }
    }
}

/// The session's worker: its takes in the order they ended, each filed before the next begins.
pub(super) async fn file_all(
    me: std::sync::Weak<Bridge>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    session: String,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Filing>,
) {
    while let Some(job) = rx.recv().await {
        let Some(b) = me.upgrade() else {
            // The bridge went with takes still queued: each is said, never dropped in silence.
            rx.close();
            let mut left = vec![job];
            while let Some(j) = rx.recv().await {
                left.push(j);
            }
            for j in left {
                log(&format!(
                    "[rec] {}: recorded {}, not filed — the front shut down first",
                    j.take.track, j.take.file
                ));
            }
            return;
        };
        b.file_now(&session, job).await;
    }
}

impl Tells for Bridge {
    fn recorded(&self, session: &str, take: Take) {
        self.file_take(session, take);
    }

    /// No runtime listens to the engine's frames any more: a listened frame goes nowhere.
    fn forward(&self, _session: &str, _kind: &str, _frame: &Value) {}

    fn say(&self, line: &str) {
        (self.log)(line);
    }
}
