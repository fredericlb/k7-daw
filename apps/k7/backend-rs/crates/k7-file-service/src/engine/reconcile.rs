//! The poll's check that the engine still holds what a session was sent (2225), and the re-send or re-dial when it does
//! not: a silent socket is re-dialled, a deaf one too after three re-sends. Sans I/O: the poll asks, the answer comes back.
//! What a reader gets wrong: the throttle is per session, so one person's poll cannot eat another's turn.

use std::time::{Duration, Instant};

use super::{Effect, Event, Phase};
use crate::paths::basename;

/// How often the poll may re-ask the engine what it holds. Polled every 2 s, per tab.
pub const RECONCILE_EVERY: Duration = Duration::from_secs(5);
/// Re-sends the engine may ignore before the socket is presumed deaf.
pub const RESEND_LIMIT: u32 = 3;
/// How long the engine gets to answer before its link is treated as dead.
pub const SESSIONS_TIMEOUT: Duration = Duration::from_secs(2);

/// The session's socket, as the front found it when the poll came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// No engine is selected, or none could be dialled.
    None,
    /// Dialled, not open yet: it re-sends from its own `open`, and asking now would time out on the wait, not the engine.
    Dialling,
    Ready,
}

pub struct Reconcile {
    session: String,
    last: Option<Instant>,
    reconciling: bool,
    asking: Option<Instant>,
    serving: String,
    /// Consecutive re-sends the engine did not take: a live socket can still be deaf.
    pub resends: u32,
}

impl Reconcile {
    pub fn new(session: &str) -> Reconcile {
        Reconcile {
            session: session.to_string(),
            last: None,
            reconciling: false,
            asking: None,
            serving: String::new(),
            resends: 0,
        }
    }

    /// Whether the engine has been asked and has not answered yet.
    pub fn asking(&self) -> bool {
        self.asking.is_some()
    }

    /// The poll came: whether it may ask the engine. Held-back and scanning sessions, and a poll too soon, are left alone,
    /// and the front dials nothing for them.
    pub fn begin(&mut self, now: Instant, held_back: bool, phase: Option<Phase>) -> bool {
        if self.reconciling
            || self
                .last
                .is_some_and(|l| now.saturating_duration_since(l) < RECONCILE_EVERY)
        {
            return false;
        }
        // A document held back by a copy or by the scan is not on the engine, and that is no fault of the engine's.
        if held_back || phase == Some(Phase::Scanning) {
            return false;
        }
        self.last = Some(now);
        self.reconciling = true;
        true
    }

    /// The front found the session's socket: ask the engine, unless it is still dialling or a load asks it itself.
    pub fn ask(
        &mut self,
        now: Instant,
        project: &str,
        link: LinkState,
        phase: Option<Phase>,
    ) -> Vec<Effect> {
        let busy = matches!(phase, Some(Phase::Applying | Phase::Warming));
        if link != LinkState::Ready || busy {
            self.reconciling = false;
            return vec![];
        }
        self.serving = basename(project);
        self.asking = Some(now + SESSIONS_TIMEOUT);
        vec![Effect::AskLoaded { run: None }]
    }

    pub fn on(&mut self, e: Event) -> Vec<Effect> {
        match e {
            Event::Loaded { run: None, held } if self.asking.is_some() => self.answered(held),
            Event::Tick(at) if self.asking.is_some_and(|d| at >= d) => self.answered(None),
            _ => vec![],
        }
    }

    fn answered(&mut self, held: Option<bool>) -> Vec<Effect> {
        self.asking = None;
        self.reconciling = false;
        let sid = &self.session;
        match held {
            None => vec![
                Effect::Log(format!(
                    "   \u{2717} engine link unresponsive \u{2014} re-dialling {sid}"
                )),
                Effect::Redial,
            ],
            Some(true) => {
                self.resends = 0;
                vec![]
            }
            Some(false) => {
                self.resends += 1;
                // A socket that answers but never takes the project is deaf, not busy.
                if self.resends > RESEND_LIMIT {
                    self.resends = 0;
                    vec![
                        Effect::Log(format!(
                            "   \u{2717} engine took {RESEND_LIMIT} re-sends without holding {} \u{2014} re-dialling {sid}",
                            self.serving
                        )),
                        Effect::Redial,
                    ]
                } else {
                    vec![
                        Effect::Log(format!(
                            "   \u{26a0} engine is not holding {} \u{2014} re-sending it ({}/{RESEND_LIMIT})",
                            self.serving, self.resends
                        )),
                        Effect::Resend,
                    ]
                }
            }
        }
    }
}
