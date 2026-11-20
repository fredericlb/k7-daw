//! A session's load (2220): copied, applied, then a dry run, half a second played with the master at silence so the
//! first real play finds every plugin running. The TypeScript's `engineLoad.ts` as a machine over events and a clock.
//! What a reader gets wrong: a replaced load's run lives on until its next answer, as the promise did (docs/engineLoad.md).

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::wire::{project, record_enable, transport};
use super::{Effect, Event, Phase};

/// How long the engine gets to apply the project, plugins included, before the gate lets go anyway.
const APPLY_TIMEOUT: Duration = Duration::from_secs(120);
/// How often the engine is asked whether it holds the project yet.
const POLL: Duration = Duration::from_millis(300);
/// How long the dry run plays.
const DRY_RUN: Duration = Duration::from_millis(500);
/// Kept silent after the stop, so a reverb's or a delay's tail is not heard when the master comes back.
const TAIL: Duration = Duration::from_millis(1500);
/// How long one question to the engine waits for its answer, as the TypeScript's `awaitSessions`.
const ASK_TIMEOUT: Duration = Duration::from_secs(2);
/// -100 dB is exactly zero on the engine's master gain.
const SILENT_DB: i64 = -100;

#[derive(Debug, Clone, Copy)]
enum Step {
    /// Asked, and answered or given up on by this instant.
    Asking(Instant),
    Polling(Instant),
    Dry(Instant),
    Tail(Instant),
}

/// One `warmUp` in flight: it outlives its entry when another load replaces it, and ends at its next answer.
#[derive(Debug)]
struct Run {
    id: u64,
    link: u64,
    step: Step,
    deadline: Instant,
    played: bool,
    stopped: bool,
    record_was_on: bool,
    gone: bool,
    /// The document that started this run, which its dry run replays silenced.
    doc: Value,
}

/// The session's entry in the table of loads: its phase and the link and run that hold it.
#[derive(Debug)]
struct Entry {
    phase: Phase,
    engine: Option<u64>,
    run: Option<u64>,
}

#[derive(Default)]
pub struct Load {
    /// Whether any engine is selected: with none, nothing will ever answer and the gate lets go.
    pub has_engine: bool,
    /// Whether rolling records, as the link read it from the last `state` frame.
    pub record_enabled: bool,
    /// The workspace's folder name, which prefixes the console lines the load says.
    pub label: String,
    entry: Option<Entry>,
    runs: Vec<Run>,
    next: u64,
    /// The clock as the last call saw it, for the calls that carry none.
    seen: Option<Instant>,
}

impl Load {
    pub fn new() -> Load {
        Load {
            has_engine: true,
            ..Load::default()
        }
    }

    /// A flush switches record off at the engine, whatever the last `state` frame said.
    pub fn flushed(&mut self) {
        self.record_enabled = false;
    }

    /// Whether any run still has a deadline to watch: the front keeps its clock turning while so.
    pub fn busy(&self) -> bool {
        !self.runs.is_empty()
    }

    pub fn phase(&self) -> Option<Phase> {
        self.entry.as_ref().map(|e| e.phase)
    }

    /// Whether documents must be held back: the engine plays a silenced copy, which the restore replaces.
    pub fn is_warming(&self) -> bool {
        self.phase() == Some(Phase::Warming)
    }

    /// A project was opened in the session: the gate waits until the load ends. The replaced dry run's stop comes first.
    pub fn begin(&mut self) -> Vec<Effect> {
        self.replace(Some(Entry {
            phase: Phase::Copying,
            engine: None,
            run: None,
        }))
    }

    /// Stop waiting: a failed copy, no engine at all, the session unloaded.
    pub fn end(&mut self) -> Vec<Effect> {
        self.replace(None)
    }

    /// Hold a load that has not reached the engine yet while the engine scans its plugins.
    pub fn hold_for_scan(&mut self) {
        if let Some(e) = self.entry.as_mut().filter(|e| e.phase == Phase::Copying) {
            e.phase = Phase::Scanning;
        }
    }

    /// The scan ended or was given up on: the held load goes on, true when there was one to release.
    pub fn release_scan_hold(&mut self) -> bool {
        match self.entry.as_mut().filter(|e| e.phase == Phase::Scanning) {
            Some(e) => {
                e.phase = Phase::Copying;
                true
            }
            None => false,
        }
    }

    fn clock(&self) -> Instant {
        self.seen.unwrap_or_else(Instant::now)
    }

    /// Replace the entry, cutting the run it held short first: its stop must reach the engine before anything else does.
    fn replace(&mut self, next: Option<Entry>) -> Vec<Effect> {
        let prev = std::mem::replace(&mut self.entry, next);
        match prev.and_then(|p| p.run) {
            Some(id) => self.cancel(id),
            None => vec![],
        }
    }

    fn index(&self, id: u64) -> Option<usize> {
        self.runs.iter().position(|r| r.id == id)
    }

    fn mine(&self, id: u64) -> bool {
        self.entry.as_ref().is_some_and(|e| e.run == Some(id))
    }

    /// The dry run's one stop, whoever asks first: its own timer, or the load that replaced it.
    fn stop(&mut self, i: usize) -> Vec<Effect> {
        let r = &mut self.runs[i];
        if !r.played || r.stopped {
            return vec![];
        }
        r.stopped = true;
        let mut fx = vec![Effect::Stop];
        if r.record_was_on {
            fx.push(Effect::Send(record_enable(true)));
        }
        fx
    }

    /// The `finally` of a warm-up: stop, let go of the entry if it is still ours, put the master back.
    fn finish(&mut self, i: usize) -> Vec<Effect> {
        let mut fx = self.stop(i);
        let run = self.runs.remove(i);
        let ours = self.mine(run.id);
        if ours {
            self.entry = None;
        }
        // Un-silences the master, unless a newer load holds the session: its own document does that.
        if ours || (run.played && self.entry.is_none()) {
            fx.push(Effect::Restore);
        }
        fx
    }

    /// A run whose entry was replaced or ended: stop it at once and wake its waits.
    fn cancel(&mut self, id: u64) -> Vec<Effect> {
        let Some(i) = self.index(id) else {
            return vec![];
        };
        let mut fx = self.stop(i);
        match self.runs[i].step {
            // A woken wait returns at once, so the loop asks again before it finds the entry gone.
            Step::Polling(_) => {
                self.runs[i].step = Step::Asking(self.clock() + ASK_TIMEOUT);
                fx.push(Effect::AskLoaded { run: Some(id) });
            }
            Step::Dry(_) | Step::Tail(_) => fx.extend(self.finish(i)),
            Step::Asking(_) => {}
        }
        fx
    }

    /// `doc` just reached the engine through `link`: wait for it to be applied, then run the dry run on it once.
    pub fn warm_up(&mut self, link: u64, doc: Value, now: Instant) -> Vec<Effect> {
        self.seen = Some(now);
        let take_over = self
            .entry
            .as_ref()
            .is_some_and(|e| e.phase == Phase::Applying && e.engine != Some(link));
        let fresh = self
            .entry
            .as_ref()
            .is_some_and(|e| e.phase == Phase::Copying);
        if !fresh && !take_over {
            return vec![];
        }
        let id = self.next;
        self.next += 1;
        self.entry = Some(Entry {
            phase: Phase::Applying,
            engine: Some(link),
            run: Some(id),
        });
        self.runs.push(Run {
            id,
            link,
            step: Step::Asking(now + ASK_TIMEOUT),
            deadline: now + APPLY_TIMEOUT,
            played: false,
            stopped: false,
            record_was_on: false,
            gone: false,
            doc,
        });
        vec![Effect::AskLoaded { run: Some(id) }]
    }

    pub fn on(&mut self, e: Event, now: Instant) -> Vec<Effect> {
        self.seen = Some(now);
        match e {
            // An answer belongs to the run that asked: a takeover leaves two asking, and a swapped answer is not the other's.
            Event::Loaded {
                run: Some(id),
                held,
            } => match self
                .runs
                .iter()
                .position(|r| r.id == id && matches!(r.step, Step::Asking(_)))
            {
                Some(i) => self.answered(i, held == Some(true), now),
                None => vec![],
            },
            Event::Tick(at) => self.tick(at),
            Event::Closed { link } => {
                for r in self.runs.iter_mut().filter(|r| r.link == link) {
                    r.gone = true;
                }
                vec![]
            }
            Event::SendFailed(msg) => self.send_failed(&msg),
            _ => vec![],
        }
    }

    fn say(&self, msg: &str) -> Effect {
        Effect::Log(format!("   \u{26a0} {}: {msg}", self.label))
    }

    /// The loop's `while (!(await loaded()))`, one answer later; `i` may be removed by it.
    fn answered(&mut self, i: usize, loaded: bool, now: Instant) -> Vec<Effect> {
        let id = self.runs[i].id;
        if !self.mine(id) {
            return self.finish(i);
        }
        if loaded {
            return self.warm(i, now);
        }
        if self.runs[i].gone {
            // The engine changed under it: wait for the new one's document, or let the gate go if there is none.
            self.entry = self.has_engine.then_some(Entry {
                phase: Phase::Copying,
                engine: None,
                run: None,
            });
            self.runs.remove(i);
            return vec![];
        }
        if now > self.runs[i].deadline {
            let mut fx = vec![self.say("the engine did not report the project loaded in time")];
            fx.extend(self.finish(i));
            return fx;
        }
        self.runs[i].step = Step::Polling(now + POLL);
        vec![]
    }

    /// The engine holds the project: send it silenced, record off, play.
    fn warm(&mut self, i: usize, now: Instant) -> Vec<Effect> {
        if let Some(e) = self.entry.as_mut() {
            e.phase = Phase::Warming;
        }
        let mut doc = self.runs[i].doc.clone();
        if let Value::Object(o) = &mut doc {
            o.insert("masterVolume".into(), json!(SILENT_DB));
        }
        let r = &mut self.runs[i];
        // Record off whatever the link read: a press younger than the last state frame is on at the engine.
        r.record_was_on = self.record_enabled;
        r.played = true;
        r.step = Step::Dry(now + DRY_RUN);
        vec![
            Effect::Send(project(doc)),
            Effect::Send(record_enable(false)),
            Effect::Send(transport("play")),
        ]
    }

    fn tick(&mut self, now: Instant) -> Vec<Effect> {
        let mut fx = vec![];
        let mut i = 0;
        while i < self.runs.len() {
            let before = self.runs.len();
            match self.runs[i].step {
                // An ask the engine never answered is an answer of "no": the next one follows after the pause.
                Step::Asking(due) if now >= due => fx.extend(self.answered(i, false, now)),
                Step::Polling(due) if now >= due => {
                    let id = self.runs[i].id;
                    self.runs[i].step = Step::Asking(now + ASK_TIMEOUT);
                    fx.push(Effect::AskLoaded { run: Some(id) });
                }
                Step::Dry(due) if now >= due => {
                    fx.extend(self.stop(i));
                    let id = self.runs[i].id;
                    // Replaced: the next load's document is on its way, and a tail waited out here would land in the middle of it.
                    if self.mine(id) {
                        self.runs[i].step = Step::Tail(now + TAIL);
                    } else {
                        fx.extend(self.finish(i));
                    }
                }
                Step::Tail(due) if now >= due => fx.extend(self.finish(i)),
                _ => {}
            }
            if self.runs.len() == before {
                i += 1;
            }
        }
        fx
    }

    /// A send of the warm step failed: nothing was played, the master is put back.
    fn send_failed(&mut self, msg: &str) -> Vec<Effect> {
        let Some(i) = self
            .runs
            .iter()
            .position(|r| matches!(r.step, Step::Dry(_)) && !r.stopped && self.mine(r.id))
        else {
            return vec![];
        };
        self.runs[i].played = false;
        let mut fx = vec![self.say(&format!("dry run: {msg}"))];
        fx.extend(self.finish(i));
        fx
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_load_nobody_began_is_not_warmed() {
        let mut l = Load::new();
        assert!(l.warm_up(1, Value::Null, Instant::now()).is_empty());
        assert_eq!(l.phase(), None);
    }
}
