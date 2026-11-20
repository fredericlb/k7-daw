//! The scan wait (2222): a project being loaded that names an external plugin waits for the engine's plugin scan, so it
//! goes out compiled with the plugins installed since. A stalled or silent scan lets it go, as before. Sans I/O.
//! What a reader gets wrong: the hold is the LOAD's phase; this machine only decides when to release it and resend.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::load::Load;
use super::{Effect, Event, ScanStatus};

/// No word from a running scan for this long, and it is taken for hung: one plugin's load is bounded at 23 s.
pub const SCAN_STALL: Duration = Duration::from_secs(60);
/// However much it says, a scan held for longer than this lets the project go.
pub const SCAN_MAX: Duration = Duration::from_secs(10 * 60);
/// An engine that has not answered `plugin-scan` by then is older than the question.
pub const SCAN_ASK: Duration = Duration::from_secs(3);
/// How long a project that went on without its scan says so.
const STALLED_SHOWN: Duration = Duration::from_secs(30);
/// How long the catalogue the scan ended on is waited for, when its broadcast was missed.
const CATALOGUE: Duration = Duration::from_secs(5);
/// Projects whose external-ness is remembered, by revision.
const MEMO: usize = 64;

struct Wait {
    project: String,
    link: u64,
    status: Option<ScanStatus>,
    since: Instant,
    heard_at: Instant,
    catalogue: Option<Value>,
    ask_due: Option<Instant>,
    next_tick: Instant,
}

struct GaveUp {
    link: u64,
    version: i64,
    at: Instant,
    shown: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Why {
    Ended,
    Silent,
    Stalled,
    Closed,
}

/// What the poll says of the scan: the one waited for, or the one given up on lately.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanView {
    pub done: u64,
    pub total: u64,
    pub current: String,
    pub stalled: bool,
}

impl ScanView {
    pub fn to_json(&self) -> Value {
        let mut o =
            serde_json::json!({ "done": self.done, "total": self.total, "current": self.current });
        if self.stalled {
            o["stalled"] = Value::Bool(true);
        }
        o
    }
}

#[derive(Default)]
pub struct Scan {
    wait: Option<Wait>,
    gave_up: Option<GaveUp>,
    /// The catalogue asked for after the scan ended: the link it was asked on, and when to stop waiting.
    finishing: Option<Instant>,
    classified: Vec<(String, String, bool)>,
}

fn version_of(s: Option<&ScanStatus>) -> i64 {
    s.map_or(-1, |s| s.version)
}

impl Scan {
    pub fn new() -> Scan {
        Scan::default()
    }

    /// Whether `project` is external at `rev`, read once per revision since it reads the whole project.
    fn names_external(
        &mut self,
        project: &str,
        rev: &str,
        external: impl FnOnce() -> bool,
    ) -> bool {
        if let Some((_, r, e)) = self.classified.iter().find(|(p, _, _)| p == project)
            && r == rev
        {
            return *e;
        }
        let e = external();
        match self.classified.iter_mut().find(|(p, _, _)| p == project) {
            Some(slot) => *slot = (project.into(), rev.into(), e),
            None => self.classified.push((project.into(), rev.into(), e)),
        }
        if self.classified.len() > MEMO {
            self.classified.remove(0);
        }
        e
    }

    /// What the memo says of `project` at `rev`, so the front reads the project on the blocking pool only when it must.
    pub fn memoised(&self, project: &str, rev: &str) -> Option<bool> {
        self.classified
            .iter()
            .find(|(p, r, _)| p == project && r == rev)
            .map(|(_, _, e)| *e)
    }

    /// Whether a wait or a catalogue is still under a clock.
    pub fn busy(&self) -> bool {
        self.wait.is_some() || self.finishing.is_some()
    }

    /// Whether the load of `(project, revision)` waits for `link`'s scan; the effects ask the scan if the link has none yet.
    /// `external` reads the whole project, synchronously wherever it is called: the front calls it on the blocking pool,
    /// outside the link's lock, after `memoised` found nothing (F4).
    pub fn holds(
        &mut self,
        load: &mut Load,
        (project, rev): (&str, &str),
        link: u64,
        link_scan: Option<&ScanStatus>,
        external: impl FnOnce() -> bool,
        now: Instant,
    ) -> (bool, Vec<Effect>) {
        if let Some(w) = &self.wait {
            if w.project == project && w.link == link {
                return (true, vec![]);
            }
            // Another project or another engine: the old wait goes, and its hold with it, before this one is judged.
            self.wait = None;
            load.release_scan_hold();
        }
        if let Some(g) = &self.gave_up
            && g.link == link
            && g.version == version_of(link_scan)
        {
            return (false, vec![]);
        }
        if link_scan.is_some_and(|s| !s.scanning) {
            return (false, vec![]);
        }
        if !self.names_external(project, rev, external) {
            return (false, vec![]);
        }
        let mut fx = vec![];
        load.hold_for_scan();
        if link_scan.is_none() {
            fx.push(Effect::AskScan);
        }
        self.wait = Some(Wait {
            project: project.into(),
            link,
            status: link_scan.cloned(),
            since: now,
            heard_at: now,
            catalogue: None,
            ask_due: link_scan.is_none().then_some(now + SCAN_ASK),
            next_tick: now + Duration::from_secs(1),
        });
        (true, fx)
    }

    /// What the poll says of the session's scan.
    pub fn view(&self, now: Instant) -> Option<ScanView> {
        if let Some(w) = &self.wait {
            let s = w.status.as_ref();
            return Some(ScanView {
                done: s.map_or(0, |s| s.done),
                total: s.map_or(0, |s| s.total),
                current: s.map_or(String::new(), |s| s.current.clone()),
                stalled: false,
            });
        }
        let g = self.gave_up.as_ref()?;
        (g.shown && now.saturating_duration_since(g.at) < STALLED_SHOWN).then(|| ScanView {
            done: 0,
            total: 0,
            current: String::new(),
            stalled: true,
        })
    }

    /// Drop the wait without sending anything: the session was unloaded, or another project took the load.
    pub fn forget(&mut self) {
        self.wait = None;
    }

    pub fn waiting(&self) -> bool {
        self.wait.is_some()
    }

    fn finish(&mut self, load: &mut Load, why: Why, now: Instant) -> Vec<Effect> {
        let Some(w) = self.wait.take() else {
            return vec![];
        };
        let mut fx = vec![];
        match why {
            Why::Silent | Why::Stalled => {
                self.gave_up = Some(GaveUp {
                    link: w.link,
                    version: version_of(w.status.as_ref()),
                    at: now,
                    shown: why == Why::Stalled,
                });
                if why == Why::Stalled {
                    fx.push(Effect::Log(
                        "   \u{26a0} the engine's plugin scan did not finish \u{2014} opening without it".into(),
                    ));
                }
            }
            Why::Ended => match w.catalogue {
                // The engine broadcasts the list before the end; asked for when that frame was missed.
                Some(list) => fx.push(Effect::WriteCatalogue(list)),
                None => {
                    self.finishing = Some(now + CATALOGUE);
                    return vec![Effect::AskCatalogue];
                }
            },
            Why::Closed => {}
        }
        if load.release_scan_hold() {
            fx.push(Effect::Resend);
        }
        fx
    }

    pub fn on(&mut self, load: &mut Load, e: Event, now: Instant) -> Vec<Effect> {
        match e {
            Event::Scan(s) => {
                let Some(w) = self.wait.as_mut() else {
                    return vec![];
                };
                let over = !s.scanning;
                w.status = Some(s);
                w.heard_at = now;
                if over {
                    self.finish(load, Why::Ended, now)
                } else {
                    vec![]
                }
            }
            Event::Plugins(list) => {
                if let Some(w) = self.wait.as_mut() {
                    w.catalogue = Some(list);
                    vec![]
                } else if self.finishing.take().is_some() {
                    let mut fx = vec![Effect::WriteCatalogue(list)];
                    if load.release_scan_hold() {
                        fx.push(Effect::Resend);
                    }
                    fx
                } else {
                    vec![]
                }
            }
            Event::Closed { link } => match &self.wait {
                Some(w) if w.link == link => self.finish(load, Why::Closed, now),
                _ => vec![],
            },
            // An engine older than the question answers it with an error, which says as much as silence, sooner.
            Event::EngineError(m) => match &self.wait {
                Some(w) if m.contains("plugin-scan") && w.status.is_none() => {
                    self.finish(load, Why::Silent, now)
                }
                _ => vec![],
            },
            Event::Tick(at) => self.tick(load, at),
            _ => vec![],
        }
    }

    fn tick(&mut self, load: &mut Load, now: Instant) -> Vec<Effect> {
        if let Some(due) = self.finishing
            && now >= due
        {
            self.finishing = None;
            return if load.release_scan_hold() {
                vec![Effect::Resend]
            } else {
                vec![]
            };
        }
        let Some(w) = self.wait.as_mut() else {
            return vec![];
        };
        if w.ask_due.is_some_and(|d| now >= d) {
            w.ask_due = None;
            if w.status.is_none() {
                return self.finish(load, Why::Silent, now);
            }
        }
        // One clock for both bounds: no word for a minute, or ten minutes in all.
        if now >= w.next_tick {
            if now.saturating_duration_since(w.heard_at) > SCAN_STALL
                || now.saturating_duration_since(w.since) > SCAN_MAX
            {
                return self.finish(load, Why::Stalled, now);
            }
            w.next_tick = now + Duration::from_secs(1);
        }
        vec![]
    }
}
