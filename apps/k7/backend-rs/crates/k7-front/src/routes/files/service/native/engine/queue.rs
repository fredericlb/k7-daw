//! One session: its machines, its socket and the ordered queue every frame leaves through (2216). `State` is under a
//! std Mutex; a frame is pushed onto the socket's channel while it is held, which is what makes the order the lock's.
//! What a reader gets wrong: nothing here awaits; what must wait (a compile, a stat, a job) is `flow.rs`, outside the lock.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::load::Load;
use k7_file_service::engine::reconcile::{LinkState, Reconcile};
use k7_file_service::engine::scan::Scan;
use k7_file_service::engine::wire::{
    In, Take, drop_session, flush, list_plugins, list_sessions, plugin_scan, record_enable, stamp,
    transport,
};
use k7_file_service::engine::{Effect, Event, Phase, ScanStatus};
use serde_json::{Value, json};

use super::hold::Hold;
use super::socket::{Handler, Input, Socket};
use super::stats::Stats;
use super::{Inner, held};

/// What a session serves: the workspace's folder and the project in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Served {
    pub root: String,
    pub project: String,
}

/// What an effect asks that cannot be done under the lock.
pub(crate) enum After {
    /// The scan ended: the project is compiled anew (`invalidate`) and sent again.
    Recompile,
    /// The engine does not hold the project: send the session's document again (counted).
    Resend,
    /// The dry run is over: the document goes again at its own master level.
    Restore,
    Catalogue(Value),
    Redial,
    Log(String),
    Recorded(Take),
    Forward(String, Value),
    Opened,
}

pub(crate) struct State {
    pub load: Load,
    pub scan: Scan,
    pub reconcile: Reconcile,
    pub served: Option<Served>,
    pub client_doc: Option<Value>,
    pub sent_to: Option<String>,
    pub socket: Option<Socket>,
    pub link_scan: Option<ScanStatus>,
    pub playing: bool,
    /// The asks of `list-sessions` awaiting an answer: a load's run, or none for the reconcile.
    pub pending: Vec<Option<u64>>,
    pub listens: HashSet<String>,
    pub take_arm: Option<TakeArm>,
    pub audition: Option<String>,
    pub last_used: Instant,
    pub untyped: u64,
    pub patterns: Option<Box<dyn std::any::Any + Send>>,
    pub hold: Hold,
    /// A poll's prewarm is running: the next polls start none (64 polls make one task).
    pub prewarming: bool,
}

pub struct Session {
    pub(crate) name: String,
    me: Weak<Session>,
    inner: Weak<Inner>,
    pub(crate) st: Mutex<State>,
    /// One document at a time through the copy gate and the send, in the order asked.
    pub(crate) gate: tokio::sync::Mutex<()>,
    #[cfg(test)]
    framed: std::sync::atomic::AtomicUsize,
}

impl Session {
    pub(crate) fn new(name: &str, inner: Weak<Inner>) -> Arc<Session> {
        Arc::new_cyclic(|me| Session {
            name: name.to_string(),
            me: me.clone(),
            inner,
            st: Mutex::new(State {
                load: Load::new(),
                scan: Scan::new(),
                reconcile: Reconcile::new(name),
                served: None,
                client_doc: None,
                sent_to: None,
                socket: None,
                link_scan: None,
                playing: false,
                pending: vec![],
                listens: HashSet::new(),
                take_arm: None,
                audition: None,
                last_used: Instant::now(),
                untyped: 0,
                patterns: None,
                hold: Hold::default(),
                prewarming: false,
            }),
            gate: tokio::sync::Mutex::new(()),
            #[cfg(test)]
            framed: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn inner(&self) -> Option<Arc<Inner>> {
        self.inner.upgrade()
    }

    pub(crate) fn me(&self) -> Arc<Session> {
        self.me.upgrade().expect("a session is held by its link")
    }

    pub fn serving(&self) -> Option<String> {
        held(&self.st).served.as_ref().map(|s| s.project.clone())
    }

    pub fn served(&self) -> Option<Served> {
        held(&self.st).served.clone()
    }

    pub fn has_socket(&self) -> bool {
        held(&self.st).socket.is_some()
    }

    /// Frames handed to `frame`, counted at the call whatever path they then take (a test's check that one was queued before an answer).
    #[cfg(test)]
    pub fn queued(&self) -> usize {
        self.framed.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_ready(&self) -> bool {
        held(&self.st).socket.as_ref().is_some_and(Socket::is_ready)
    }

    /// Whether the socket is open, dialling it first when the session has none (2273: a verb asks this before any frame goes).
    pub fn dial_ready(&self) -> bool {
        let mut st = held(&self.st);
        self.ensure_socket(&mut st);
        st.socket.as_ref().is_some_and(Socket::is_ready)
    }

    pub fn playing(&self) -> bool {
        held(&self.st).playing
    }

    pub fn load_phase(&self) -> Option<Phase> {
        held(&self.st).load.phase()
    }

    /// The link's number, kept across reconnects of its socket (F3).
    pub fn link_no(&self) -> Option<u64> {
        held(&self.st).socket.as_ref().map(|s| s.link)
    }

    pub fn idle_since(&self, now: Instant) -> Duration {
        now.saturating_duration_since(held(&self.st).last_used)
    }

    /// Listen for a frame type the stand-in forwards; `state` is refused (2217).
    pub fn listen(&self, kinds: &[String]) {
        let mut st = held(&self.st);
        for k in kinds.iter().filter(|k| k.as_str() != "state") {
            st.listens.insert(k.clone());
        }
    }

    pub fn set_take_arm(&self, arm: Option<TakeArm>) {
        held(&self.st).take_arm = arm;
    }

    pub fn set_audition(&self, id: Option<String>) {
        held(&self.st).audition = id;
    }

    /// Queue a frame the verbs built, in the order asked; false when no engine is bound (2216).
    pub fn frame(&self, frame: Value) -> bool {
        #[cfg(test)]
        self.framed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut st = held(&self.st);
        st.last_used = Instant::now();
        if Session::jobs_pending(&st) {
            drop(st);
            self.me().enqueue_frame(frame);
            return true;
        }
        if Session::hold_frame(&mut st, &frame) {
            return true;
        }
        self.push(&mut st, frame)
    }

    /// Stamp and queue `frame` as it is (a document is prepared once, by `dispatch`), dialling the session's socket if it has none. False with no engine bound.
    pub(crate) fn push(&self, st: &mut State, frame: Value) -> bool {
        if self.ensure_socket(st).is_none() {
            return false;
        }
        let stamped = stamp(&self.name, frame);
        st.socket.as_ref().is_some_and(|s| s.send(stamped))
    }

    /// The socket of this session, dialled on first use; none while no engine is bound.
    pub(crate) fn ensure_socket(&self, st: &mut State) -> Option<u64> {
        if let Some(s) = &st.socket {
            return Some(s.link);
        }
        let inner = self.inner()?;
        let url = inner.binding_url()?;
        let link = inner.new_link();
        let weak = self.me.clone();
        let handler: Handler = Arc::new(move |input| {
            if let Some(s) = weak.upgrade() {
                s.on_input(link, input);
            }
        });
        st.link_scan = None;
        st.playing = false;
        st.socket = Some(Socket::dial(
            &inner.rt,
            url,
            self.name.clone(),
            link,
            handler,
        ));
        Some(link)
    }

    /// Apply a machine's effects: frames under the lock, the rest handed back for after it. `scan` marks the scan's own.
    pub(crate) fn apply(&self, st: &mut State, fx: Vec<Effect>, scan: bool) -> Vec<After> {
        let mut after = vec![];
        for e in fx {
            match e {
                Effect::Send(f) => {
                    if !self.push(st, f) {
                        let more = st
                            .load
                            .on(Event::SendFailed("no engine link".into()), Instant::now());
                        after.extend(self.apply(st, more, false));
                    }
                }
                Effect::Stop => {
                    self.push(st, transport("stop"));
                    self.push(st, transport("rewind"));
                }
                Effect::Restore => after.push(After::Restore),
                Effect::AskLoaded { run } => {
                    if self.push(st, list_sessions()) {
                        st.pending.push(run);
                    }
                }
                Effect::AskScan => {
                    self.push(st, plugin_scan());
                }
                Effect::AskCatalogue => {
                    self.push(st, list_plugins());
                }
                Effect::WriteCatalogue(list) => after.push(After::Catalogue(list)),
                Effect::Resend => {
                    if scan {
                        after.push(After::Recompile);
                    }
                    after.push(After::Resend);
                }
                Effect::Redial => after.push(After::Redial),
                Effect::Log(l) => after.push(After::Log(l)),
            }
        }
        after
    }

    /// Feed one event to every machine, the way the TypeScript's callbacks reach whichever waits on it.
    pub(crate) fn feed(&self, st: &mut State, e: Event, now: Instant) -> Vec<After> {
        let fx = st.load.on(e.clone(), now);
        let mut after = self.apply(st, fx, false);
        let State { load, scan, .. } = &mut *st;
        let fx = scan.on(load, e.clone(), now);
        after.extend(self.apply(st, fx, true));
        let fx = st.reconcile.on(e);
        after.extend(self.apply(st, fx, false));
        after
    }

    /// What the socket said.
    fn on_input(&self, link: u64, input: Input) {
        let now = Instant::now();
        let mut st = held(&self.st);
        // A word from a socket the session has since let go of is stale.
        if st.socket.as_ref().map(|s| s.link) != Some(link) {
            return;
        }
        let after = match input {
            Input::Open => [After::Opened]
                .into_iter()
                .chain(Self::told(&st, "open", Value::Null))
                .collect(),
            Input::Down => Self::told(&st, "close", Value::Null).into_iter().collect(),
            Input::Error(m) => vec![After::Log(format!(
                "   \u{2717} engine socket {}: {m}",
                self.name
            ))],
            Input::Frame(f) => self.frame_in(&mut st, f, now),
        };
        drop(st);
        self.after(after);
    }

    fn frame_in(&self, st: &mut State, f: In, now: Instant) -> Vec<After> {
        match f {
            In::State {
                playing,
                record_enabled,
            } => {
                st.playing = playing;
                st.load.record_enabled = record_enabled;
                vec![]
            }
            In::Scan(s) => {
                if !s.scanning
                    && let Some(i) = self.inner()
                {
                    i.waiters
                        .settle("scan-ended", &json!({ "type": "scan-ended" }));
                }
                st.link_scan = Some(s.clone());
                self.feed(st, Event::Scan(s), now)
            }
            In::Sessions(rows) => {
                let held_here = rows.iter().any(|r| r.id == self.name && r.loaded);
                let asked = std::mem::take(&mut st.pending);
                let mut after = vec![];
                for run in asked {
                    let e = Event::Loaded {
                        run,
                        held: Some(held_here),
                    };
                    after.extend(self.feed(st, e, now));
                }
                after
            }
            In::Plugins(list) => {
                if let Some(i) = self.inner() {
                    i.waiters
                        .settle("plugins", &json!({ "type": "plugins", "plugins": list }));
                }
                let told = Self::told(st, "plugins", json!({ "type": "plugins", "plugins": list }));
                let mut after = self.feed(st, Event::Plugins(list), now);
                after.extend(told);
                after
            }
            In::Recorded(take) => vec![After::Recorded(take)],
            In::Error(m) => {
                if let Some(i) = self.inner() {
                    i.waiters
                        .settle("error", &json!({ "type": "error", "message": m }));
                }
                let told = Self::told(st, "error", json!({ "type": "error", "message": m }));
                let mut after = self.feed(st, Event::EngineError(m), now);
                after.extend(told);
                after
            }
            In::Other(kind, frame) => {
                if let Some(i) = self.inner() {
                    i.waiters.settle(&kind, &frame);
                }
                if kind != "state" && st.listens.contains(&kind) {
                    vec![After::Forward(kind, frame)]
                } else {
                    vec![]
                }
            }
            In::Ignored => vec![],
        }
    }

    /// A frame the stand-in listens for, to be forwarded (2217); none for `state`.
    fn told(st: &State, kind: &str, frame: Value) -> Option<After> {
        (kind != "state" && st.listens.contains(kind))
            .then(|| After::Forward(kind.to_string(), frame))
    }

    /// The machines' clock; true while anything of this session still has a deadline.
    pub(crate) fn tick(&self, now: Instant) -> bool {
        let mut st = held(&self.st);
        let after = self.feed(&mut st, Event::Tick(now), now);
        let busy = st.load.busy() || st.scan.busy() || st.reconcile.asking();
        drop(st);
        self.after(after);
        busy
    }

    /// Wake the ticker: a machine has a deadline now.
    pub(crate) fn wake(&self) {
        if let Some(i) = self.inner() {
            i.wake.notify_one();
        }
    }

    /// Let go of the socket: every machine hears its link is gone, and what it was serving stays served.
    pub fn drop_socket(&self) {
        let now = Instant::now();
        let mut st = held(&self.st);
        let Some(old) = st.socket.take() else { return };
        st.pending.clear();
        let link = old.link;
        drop(old);
        let after = self.feed(&mut st, Event::Closed { link }, now);
        drop(st);
        self.after(after);
        self.wake();
    }

    /// A reap of an abandoned socket: never one that serves a project, which is idle between commands by nature.
    pub(crate) fn drop_if_unserved(&self) -> bool {
        {
            let st = held(&self.st);
            if st.served.is_some() || st.socket.is_none() {
                return false;
            }
        }
        self.drop_socket();
        true
    }

    /// Tell the engine to forget the session and close the socket once what is queued is written.
    pub(crate) fn drop_session_frame(&self, st: &mut State) {
        self.push(st, drop_session(&self.name));
        st.pending.clear();
        if let Some(s) = st.socket.take() {
            s.finish();
        }
    }

    pub(crate) fn flush_frames(&self, st: &mut State) {
        self.push(st, flush());
        self.push(st, record_enable(false));
        // The dry run reads this to put record back: a state frame from before the switch must not be what it finds.
        st.load.flushed();
    }

    /// The reconcile's view of the socket.
    pub(crate) fn link_state(st: &State) -> LinkState {
        match &st.socket {
            None => LinkState::None,
            Some(s) if s.is_ready() => LinkState::Ready,
            Some(_) => LinkState::Dialling,
        }
    }

    /// Run what could not be done under the lock.
    pub(crate) fn after(&self, after: Vec<After>) {
        let Some(inner) = self.inner() else { return };
        self.wake();
        for a in after {
            let me = self.me();
            let i = inner.clone();
            match a {
                After::Log(l) => inner.tells.say(&l),
                After::Redial => {
                    Stats::bump(&inner.stats.redials);
                    me.redial();
                }
                After::Resend => {
                    Stats::bump(&inner.stats.resends);
                    me.enqueue(|s| async move { s.resend_served().await });
                }
                After::Restore => me.enqueue(|s| async move { s.resend_served().await }),
                After::Recompile => {
                    me.enqueue(|s| async move { s.recompile_and_resend().await });
                }
                After::Catalogue(list) => {
                    inner.spawn_blocking(move || i.host.write_catalogue(&list));
                }
                After::Recorded(take) => inner.tells.recorded(&self.name, take),
                After::Forward(kind, frame) => inner.tells.forward(&self.name, &kind, &frame),
                After::Opened => me.enqueue(|s| async move { s.opened().await }),
            }
        }
    }

    /// Drop the socket and dial a fresh one, whose open re-sends what the session serves.
    fn redial(&self) {
        self.drop_socket();
        let mut st = held(&self.st);
        self.ensure_socket(&mut st);
    }

    /// The doc a prefs frame set carries: the device, then each input pair, in that order (2215).
    pub(crate) fn prefs_frames(p: &super::Prefs) -> Vec<Value> {
        let mut out = vec![];
        if let Some((name, kind)) = &p.audio_device {
            let mut f = json!({ "type": "set-audio-device", "name": name });
            if !kind.is_empty() {
                f["deviceType"] = json!(kind);
            }
            out.push(f);
        }
        for first in &p.input_pairs {
            out.push(json!({ "type": "set-input-stereo", "channel": first, "stereo": true }));
        }
        out
    }

    /// Which phase the poll reports: a load that has begun, none otherwise.
    pub fn phase(&self) -> Option<Phase> {
        self.load_phase()
    }
}
