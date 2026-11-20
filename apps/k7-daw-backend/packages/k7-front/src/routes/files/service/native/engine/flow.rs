//! A session's flows (the TypeScript's `sendDoc`, `pushProject`, `startEngineForActive`, `unloadEngine`, the reconcile):
//! each awaits what is slow (stat, compile, a job) outside the session's lock, then decides under it.
//! What a reader gets wrong: `send_doc` and a finished copy take the session's gate, so documents keep the order asked.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use k7_file_service::engine::Phase;
use k7_file_service::engine::docs::{Sent, TakeArm, prepare};
use k7_file_service::engine::scan::ScanView;
use k7_file_service::engine::sync::SyncState;
use k7_file_service::engine::wire::{project, transport};
use k7_file_service::paths::basename;
use serde_json::Value;

use super::queue::{Served, Session};
use super::stats::Stats;
use super::{EngineLink, Inner, held};

/// `prepare`, counted: a document is prepared once, which a test holds.
fn prepared(inner: &Inner, doc: &Value, arm: Option<&TakeArm>, audition: Option<&str>) -> Sent {
    Stats::bump(&inner.stats.prepares);
    prepare(doc, arm, audition)
}

impl Session {
    /// The callback of `sendWhenSynced`: the document reaches the engine, through the load's gates.
    pub(super) async fn dispatch(self: &Arc<Self>, doc: Value) {
        let Some(inner) = self.inner() else { return };
        // The external-plugin question reads the whole project: asked here, outside the lock, once per revision (F4).
        let served = self.served();
        let ask_external = held(&self.st)
            .load
            .phase()
            .is_none_or(|p| matches!(p, Phase::Copying | Phase::Scanning));
        let mut external: Option<(String, bool)> = None;
        if let (Some(s), true) = (&served, ask_external) {
            let i = inner.clone();
            let at = s.clone();
            let rev = tokio::task::spawn_blocking(move || i.host.revision(&at.root))
                .await
                .unwrap_or_default();
            let known = held(&self.st).scan.memoised(&s.project, &rev);
            let ext = match known {
                Some(b) => b,
                None => {
                    let i = inner.clone();
                    let at = s.clone();
                    tokio::task::spawn_blocking(move || {
                        i.host.names_external(&at.root, &at.project)
                    })
                    .await
                    .unwrap_or(false)
                }
            };
            external = Some((rev, ext));
        }
        // Prepared once, off the lock: an armed take hears the lane around its punch; an auditioned timeline plays alone.
        let (arm, audition) = {
            let st = held(&self.st);
            (st.take_arm.clone(), st.audition.clone())
        };
        let sent = prepared(&inner, &doc, arm.as_ref(), audition.as_deref());
        let after = {
            let mut st = held(&self.st);
            st.last_used = Instant::now();
            let now = Instant::now();
            if st.load.is_warming() {
                return;
            }
            let Some(link) = self.ensure_socket(&mut st) else {
                st.sent_to = None;
                let fx = st.load.end();
                let after = self.apply(&mut st, fx, false);
                drop(st);
                self.after(after);
                return;
            };
            let mut after = vec![];
            let url = inner.binding_url();
            if url.is_some() && st.sent_to != url && st.load.phase().is_none() {
                Stats::bump(&inner.stats.loads);
                let fx = st.load.begin();
                after.extend(self.apply(&mut st, fx, false));
            }
            let gated = matches!(st.load.phase(), Some(Phase::Copying | Phase::Scanning));
            if let (Some(s), true) = (&served, gated) {
                let (rev, ext) = external.clone().unwrap_or_default();
                let link_scan = st.link_scan.clone();
                let state = &mut *st;
                let (held_back, fx) = state.scan.holds(
                    &mut state.load,
                    (&s.project, &rev),
                    link,
                    link_scan.as_ref(),
                    || ext,
                    now,
                );
                after.extend(self.apply(&mut st, fx, true));
                if held_back {
                    drop(st);
                    self.after(after);
                    return;
                }
            }
            st.sent_to = url;
            if !sent.typed {
                st.untyped += 1;
                if st.untyped == 1 {
                    after.push(super::queue::After::Log(format!(
                        "   \u{26a0} {}: a document the typed reader refuses goes without hole or audition",
                        self.name
                    )));
                }
            }
            let queued = self.push(&mut st, project(sent.doc.clone()));
            if queued {
                let fx = st.load.warm_up(link, sent.doc, now);
                if matches!(st.load.phase(), Some(Phase::Applying)) {
                    Stats::bump(&inner.stats.dry);
                }
                after.extend(self.apply(&mut st, fx, false));
            }
            after
        };
        self.after(after);
    }

    /// `resendServed`: the client's document when it holds one, else the project compiled as it stands.
    pub async fn resend_served(self: &Arc<Self>) {
        let (doc, served) = {
            let st = held(&self.st);
            (st.client_doc.clone(), st.served.clone())
        };
        if let Some(doc) = doc {
            self.send_doc(doc).await;
        } else if let Some(s) = served {
            self.push_project(&s).await;
        }
    }

    /// The scan ended: the compile is dropped so the plugins installed since are read, then sent again.
    pub(crate) async fn recompile_and_resend(self: &Arc<Self>) {
        let (Some(inner), Some(s)) = (self.inner(), self.served()) else {
            return;
        };
        let i = inner.clone();
        let root = s.root.clone();
        let _ = tokio::task::spawn_blocking(move || i.host.invalidate(&root)).await;
        self.resend_served().await;
    }

    /// `pushProject`: compile and send, unless a browser holds the document. A compile that fails ends the wait.
    pub async fn push_project(self: &Arc<Self>, s: &Served) {
        if held(&self.st).client_doc.is_some() {
            return;
        }
        let Some(inner) = self.inner() else { return };
        // Held from before the compile: a frame asked meanwhile leaves after this document (B3).
        let _deciding = self.deciding();
        let i = inner.clone();
        let (root, p) = (s.root.clone(), s.project.clone());
        let compiled = tokio::task::spawn_blocking(move || i.host.compile(&root, &p)).await;
        match compiled {
            Ok(Ok(doc)) => {
                if held(&self.st).client_doc.is_none() {
                    self.send_doc(doc).await;
                }
            }
            Ok(Err(msg)) => self.compile_failed(&inner, &s.project, &msg),
            Err(_) => {
                Stats::bump(&inner.stats.panics);
                self.compile_failed(&inner, &s.project, "the compile panicked");
            }
        }
    }

    fn compile_failed(self: &Arc<Self>, inner: &Arc<Inner>, project: &str, msg: &str) {
        let mut st = held(&self.st);
        let fx = st.load.end();
        let after = self.apply(&mut st, fx, false);
        drop(st);
        self.after(after);
        inner.tells.say(&format!(
            "   \u{2717} could not send {} to the engine: {msg}",
            basename(project)
        ));
    }

    /// A socket opened: the preferences, then the project this session was serving (2215).
    pub(crate) async fn opened(self: &Arc<Self>) {
        let Some(inner) = self.inner() else { return };
        let i = inner.clone();
        let prefs = tokio::task::spawn_blocking(move || i.host.preferences())
            .await
            .unwrap_or_default();
        {
            let mut st = held(&self.st);
            for f in Session::prefs_frames(&prefs) {
                self.push(&mut st, f);
            }
        }
        if let Some(s) = self.served() {
            self.send_active(&s).await;
        }
    }

    /// `sendActiveProject`: note what is served first, then push; a stale late send for a vanished folder changes nothing.
    async fn send_active(self: &Arc<Self>, s: &Served) {
        if self.is_stale(&s.project).await {
            return;
        }
        held(&self.st).served = Some(s.clone());
        self.push_project(s).await;
    }

    async fn is_stale(&self, p: &str) -> bool {
        let now = self.serving();
        match now {
            Some(n) if n != p => !tokio::fs::try_exists(Path::new(p)).await.unwrap_or(false),
            _ => false,
        }
    }

    /// `startEngineForActive`: point the session at the project and send it; another project flushes the old one.
    pub async fn start(self: &Arc<Self>, root: &str, project: &str) {
        let Some(inner) = self.inner() else { return };
        if self.is_stale(project).await {
            return;
        }
        let before = self.serving();
        let switching = before.as_deref().is_some_and(|b| b != project);
        let mut after = vec![];
        {
            let mut st = held(&self.st);
            st.last_used = Instant::now();
            st.load.label = basename(root);
            // The poll that queued this start has begun the load already.
            let begun = before.is_none() && st.load.phase() == Some(Phase::Copying);
            if before.as_deref() != Some(project) && !begun {
                Stats::bump(&inner.stats.loads);
                let fx = st.load.begin();
                after.extend(self.apply(&mut st, fx, false));
            }
            if switching {
                self.flush_frames(&mut st);
                st.client_doc = None;
                st.audition = None;
            }
        }
        self.after(after);
        self.watch_patterns(root, project);
        self.send_active(&Served {
            root: root.to_string(),
            project: project.to_string(),
        })
        .await;
    }

    /// `reloadActiveProject`: an explicit reload outranks a browser's document.
    pub async fn reload(self: &Arc<Self>, root: &str, project: &str) {
        held(&self.st).client_doc = None;
        let (i, r) = (self.inner(), root.to_string());
        if let Some(i) = i {
            let h = i.clone();
            let _ = tokio::task::spawn_blocking(move || h.host.invalidate(&r)).await;
        }
        self.start(root, project).await;
    }

    /// `applyClientProject`: a document a browser built, played through the service's own link.
    pub async fn apply_client(self: &Arc<Self>, doc: Value) {
        held(&self.st).client_doc = Some(doc.clone());
        self.send_doc(doc).await;
    }

    /// `applyClientProject` at the route: the document is the client's from now on, and sent in its turn.
    pub fn apply_client_now(self: &Arc<Self>, doc: Value) {
        held(&self.st).client_doc = Some(doc.clone());
        self.push_doc(doc);
    }

    /// `forgetSyncFailure`, at the route.
    pub fn forget_failure(&self) {
        if let Some(i) = self.inner() {
            held(&i.copies).forget_failure(&self.name);
        }
    }

    /// The poll's upkeep as a job of the session's queue, one at a time (2226): the answer never waits for it, and it never overtakes a verb's start.
    pub fn prewarm_soon(self: &Arc<Self>, root: &str, active: &str) {
        let Some(inner) = self.inner() else { return };
        let after = {
            let mut st = held(&self.st);
            if st.prewarming {
                return;
            }
            st.prewarming = true;
            // The poll that starts a load already answers `copying`, as the TypeScript's synchronous start did.
            if st.served.is_none() && st.load.phase().is_none() {
                Stats::bump(&inner.stats.loads);
                let fx = st.load.begin();
                self.apply(&mut st, fx, false)
            } else {
                vec![]
            }
        };
        self.after(after);
        struct Done(Arc<Session>);
        impl Drop for Done {
            fn drop(&mut self) {
                held(&self.0.st).prewarming = false;
            }
        }
        let done = Done(self.clone());
        let (root, active) = (root.to_string(), active.to_string());
        self.enqueue(move |s| async move {
            let _done = done;
            s.prewarm(&root, &active).await;
        });
    }

    /// `retryEngineSync`: forget the failed copy, then reload.
    pub async fn retry_sync(self: &Arc<Self>, root: &str, project: &str) {
        if let Some(i) = self.inner() {
            held(&i.copies).forget_failure(&self.name);
        }
        self.reload(root, project).await;
    }

    /// `unloadEngine`: the waits ended, stop and flush, the session dropped, the watch let go.
    pub async fn unload(self: &Arc<Self>) {
        let Some(_inner) = self.inner() else { return };
        {
            let mut st = held(&self.st);
            st.scan.forget();
            let fx = st.load.end();
            let after = self.apply(&mut st, fx, false);
            if self.ensure_socket(&mut st).is_some() {
                self.push(&mut st, transport("stop"));
                self.flush_frames(&mut st);
            }
            st.served = None;
            st.sent_to = None;
            st.client_doc = None;
            st.audition = None;
            self.drop_session_frame(&mut st);
            st.patterns = None;
            drop(st);
            self.after(after);
        }
    }

    /// The poll's upkeep (`prewarmEngineForActive`): start what is not served, else check the engine still holds it.
    pub async fn prewarm(self: &Arc<Self>, root: &str, active: &str) {
        let Some(inner) = self.inner() else { return };
        let Some(served) = self.served() else {
            return self.start(root, active).await;
        };
        // Another project in the same workspace: reconciling would re-assert the one being left.
        if served.project != active {
            return;
        }
        Stats::bump(&inner.stats.polls);
        let held_back = held(&inner.copies).is_held_back(&self.name);
        let now = Instant::now();
        let after = {
            let mut st = held(&self.st);
            let phase = st.load.phase();
            if !st.reconcile.begin(now, held_back, phase) {
                return;
            }
            self.ensure_socket(&mut st);
            let ls = Session::link_state(&st);
            let fx = st.reconcile.ask(now, &served.project, ls, phase);
            self.apply(&mut st, fx, false)
        };
        self.after(after);
    }

    /// `engineLoadOf`: where the opened project stands on its way to playable, for the gate.
    pub fn engine_load_of(self: &Arc<Self>, active: Option<&str>) -> Option<Phase> {
        let inner = self.inner()?;
        let sync = held(&inner.copies).state(&self.name).cloned();
        let url = inner.binding_url();
        let mut st = held(&self.st);
        if sync.is_some_and(|s| s.failed.is_some() || s.error.is_some()) {
            let fx = st.load.end();
            let after = self.apply(&mut st, fx, false);
            drop(st);
            self.after(after);
            return None;
        }
        let serving = active.is_some() && st.served.as_ref().map(|s| s.project.as_str()) == active;
        // No engine bound: the project is on none, so Play waits.
        if serving && url.is_none() {
            st.sent_to = None;
            return Some(st.load.phase().unwrap_or(Phase::Copying));
        }
        // An engine selected after the project opened holds nothing yet.
        if serving && url.is_some() && st.load.phase().is_none() && st.sent_to != url {
            st.sent_to = url;
            Stats::bump(&inner.stats.loads);
            let fx = st.load.begin();
            let after = self.apply(&mut st, fx, false);
            let phase = st.load.phase();
            drop(st);
            self.after(after);
            self.enqueue(|s| async move { s.resend_served().await });
            return phase;
        }
        st.load.phase()
    }

    pub fn sync_state(&self) -> Option<SyncState> {
        let inner = self.inner()?;
        held(&inner.copies).state(&self.name).cloned()
    }

    pub fn scan_view(&self) -> Option<ScanView> {
        held(&self.st).scan.view(Instant::now())
    }
}

impl EngineLink {
    /// `unloadEngineIfServing`: a job in each session's queue, so a start still queued is decided before it (J3 fix 1); true when one served.
    pub async fn unload_under(&self, abs: &str) -> bool {
        let mut waits = vec![];
        for s in self.sessions() {
            let busy = {
                let st = held(&s.st);
                st.served.is_some() || Session::jobs_pending(&st)
            };
            if !busy {
                continue;
            }
            let (tx, rx) = tokio::sync::oneshot::channel();
            let abs = abs.to_string();
            s.enqueue(move |s| async move {
                let under = |p: &str| {
                    let rel = k7_file_service::paths::relative(&abs, p);
                    rel.is_empty() || !k7_file_service::paths::climbs_out(&rel)
                };
                let hit = s.serving().is_some_and(|p| under(&p));
                if hit {
                    s.unload().await;
                }
                let _ = tx.send(hit);
            });
            waits.push(rx);
        }
        let mut any = false;
        for rx in waits {
            any |= rx.await.unwrap_or(false);
        }
        any
    }
}
