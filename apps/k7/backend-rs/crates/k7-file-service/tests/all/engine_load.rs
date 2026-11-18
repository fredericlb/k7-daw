//! The load machine (2220) against the TypeScript's traces: each scenario of `vectors/engine.json`'s `load` is replayed on a
//! turned clock, through a small front that does what `sendDoc` and `warmDeps` do, and must say the same frames at the same times.

use std::collections::VecDeque;

use k7_file_service::engine::load::Load;
use k7_file_service::engine::{Effect, Event, Phase};
use serde_json::{Value, json};

use crate::support::engine::{Clock, rows, same_rows, vectors};

/// What `sendDoc` and `warmDeps` hold around a load: the link, the address last sent to, the client's document.
struct Front {
    clock: Clock,
    load: Load,
    rows: Vec<(u64, String)>,
    lines: Vec<String>,
    now: u64,
    link: Option<u64>,
    next_link: u64,
    url: Option<String>,
    sent_to: Option<String>,
    client_doc: Option<Value>,
    micro: VecDeque<(u64, Option<bool>)>,
    timed: Vec<(u64, u64, Option<bool>)>,
    loaded_at: Option<u64>,
    silent_until: Option<u64>,
    throw_on_send: bool,
}

impl Front {
    fn row(&mut self, what: String) {
        self.rows.push((self.now, what));
    }

    fn select(&mut self, url: Option<&str>) {
        // A different engine drops every link: the run that held it learns it is gone.
        if self.url.as_deref() != url
            && let Some(old) = self.link.take()
        {
            self.feed(Event::Closed { link: old });
        }
        self.url = url.map(str::to_string);
        self.load.has_engine = url.is_some();
    }

    fn feed(&mut self, e: Event) {
        let fx = self.load.on(e, self.clock.at(self.now));
        self.run(fx);
    }

    fn send_doc(&mut self, doc: Value) {
        self.client_doc = Some(doc.clone());
        self.dispatch(doc);
    }

    fn dispatch(&mut self, doc: Value) {
        if self.load.is_warming() {
            return;
        }
        let Some(url) = self.url.clone() else {
            self.sent_to = None;
            let fx = self.load.end();
            return self.run(fx);
        };
        let link = *self.link.get_or_insert_with(|| {
            self.next_link += 1;
            self.next_link
        });
        if self.sent_to.as_deref() != Some(&url) && self.load.phase().is_none() {
            let fx = self.load.begin();
            self.run(fx);
        }
        self.sent_to = Some(url);
        let failed = self.frame(&json!({ "type": "project", "doc": doc }));
        if failed {
            return;
        }
        let fx = self.load.warm_up(link, doc, self.clock.at(self.now));
        self.run(fx);
    }

    /// Say a frame as the recording client does; true when the send throws.
    fn frame(&mut self, f: &Value) -> bool {
        match f["type"].as_str().unwrap() {
            "project" => {
                let vol = f["doc"]["masterVolume"].clone();
                self.row(format!("send {vol}"));
                self.throw_on_send && vol == json!(-100)
            }
            "record-enable" => {
                self.row(format!("set-record {}", f["enabled"]));
                false
            }
            "transport" => {
                self.row(format!("transport {}", f["action"].as_str().unwrap()));
                false
            }
            other => panic!("a frame the load does not send: {other}"),
        }
    }

    fn ask(&mut self, run: u64) {
        self.row("ask".into());
        if self.silent_until.is_some_and(|s| self.now < s) {
            // A silent engine answers nothing; the machine's own 2 s bound is the answer.
            self.timed.push((self.now + 2000, run, None));
        } else {
            self.micro
                .push_back((run, Some(self.loaded_at.is_some_and(|l| self.now >= l))));
        }
    }

    fn run(&mut self, fx: Vec<Effect>) {
        for e in fx {
            match e {
                Effect::Send(f) => {
                    if self.frame(&f) {
                        // The send threw: the rest of the warm step never ran.
                        let more = self
                            .load
                            .on(Event::SendFailed("boom".into()), self.clock.at(self.now));
                        self.run(more);
                        return;
                    }
                }
                Effect::Stop => {
                    self.row("transport stop".into());
                    self.row("transport rewind".into());
                }
                Effect::Restore => {
                    if let Some(d) = self.client_doc.clone() {
                        self.dispatch(d);
                    }
                }
                Effect::AskLoaded { run } => self.ask(run.expect("a load's ask names its run")),
                Effect::Log(l) => self.lines.push(l),
                other => panic!("a load never says {other:?}"),
            }
        }
    }

    /// The microtasks of this instant: answers that were given as the ask was made.
    fn flush(&mut self) {
        while let Some((run, held)) = self.micro.pop_front() {
            self.feed(Event::Loaded {
                run: Some(run),
                held,
            });
        }
    }

    /// The timers of the window ending at `t`.
    fn advance(&mut self, t: u64) {
        self.now = t;
        let due: Vec<_> = self
            .timed
            .iter()
            .filter(|(d, _, _)| *d <= t)
            .cloned()
            .collect();
        self.timed.retain(|(d, _, _)| *d > t);
        for (_, run, held) in due {
            self.feed(Event::Loaded {
                run: Some(run),
                held,
            });
        }
        self.feed(Event::Tick(self.clock.at(t)));
        self.flush();
    }
}

fn op(f: &mut Front, name: &str, tag: usize) {
    let doc = json!({ "masterVolume": -3, "tracks": [] });
    let second = json!({ "masterVolume": -9, "tracks": [] });
    let url = |n: &str| format!("ws://127.0.0.1:1/{tag}/{n}");
    match name {
        "begin" => {
            let fx = f.load.begin();
            f.run(fx);
        }
        "warm" => f.send_doc(doc),
        "warm_other_engine" => {
            f.select(Some(&url("b")));
            f.send_doc(doc);
        }
        "end" => {
            let fx = f.load.end();
            f.run(fx);
        }
        "engine_gone" => f.select(Some(&url("gone"))),
        "engine_gone_no_engine" => f.select(None),
        "hold_scan" => f.load.hold_for_scan(),
        "release_scan" => {
            f.load.release_scan_hold();
        }
        "record_on" => f.load.record_enabled = true,
        "send_mid" => f.send_doc(second),
        "new_address" => f.select(Some(&url("c"))),
        other => panic!("an op the vectors do not have: {other}"),
    }
}

/// What a replay says: the calls, the phases as `(at, phase)` and the console's lines.
type Said = (Vec<(u64, String)>, Vec<(u64, Option<String>)>, Vec<String>);

fn replay(i: usize, case: &Value) -> Said {
    let sc = &case["script"];
    let clock = Clock::new();
    let mut load = Load::new();
    load.label = format!("load-{i}");
    let mut f = Front {
        clock,
        load,
        rows: vec![],
        lines: vec![],
        now: 0,
        link: None,
        next_link: 0,
        url: None,
        sent_to: None,
        client_doc: None,
        micro: VecDeque::new(),
        timed: vec![],
        loaded_at: sc["loadedAt"].as_u64(),
        silent_until: sc["silentUntil"].as_u64(),
        throw_on_send: sc["throwOnSend"].as_bool() == Some(true),
    };
    f.select(Some(&format!("ws://127.0.0.1:1/{i}/a")));
    let mut events: Vec<(u64, String)> = sc["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e[0].as_u64().unwrap(), e[1].as_str().unwrap().to_string()))
        .collect();
    events.sort_by_key(|e| e.0);
    let mut events: VecDeque<_> = events.into();
    let until = sc["until"].as_u64().unwrap();
    let mut phases: Vec<(u64, Option<String>)> = vec![];
    let mut t = 0;
    while t <= until {
        f.now = t;
        while events.front().is_some_and(|e| e.0 <= t) {
            let (_, name) = events.pop_front().unwrap();
            op(&mut f, &name, i);
        }
        let p = f.load.phase().map(Phase::as_str).map(str::to_string);
        if phases.last().map(|x| &x.1) != Some(&p) || phases.is_empty() {
            phases.push((t, p));
        }
        f.flush();
        f.advance(t + 100);
        t += 100;
    }
    (f.rows, phases, f.lines)
}

#[test]
fn every_trace_of_the_load_is_replayed() {
    let cases = vectors()["load"].as_array().unwrap();
    assert!(cases.len() >= 19, "{} scenarios", cases.len());
    for (i, case) in cases.iter().enumerate() {
        let name = case["name"].as_str().unwrap();
        let (calls, phases, lines) = replay(i, case);
        same_rows(name, &calls, &rows(&case["calls"]));
        let want: Vec<(u64, Option<String>)> = case["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_u64().unwrap(), p[1].as_str().map(str::to_string)))
            .collect();
        assert_eq!(phases, want, "{name}: phases");
        let want_lines: Vec<&str> = case["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(lines, want_lines, "{name}: lines");
    }
}

#[test]
fn record_is_put_back_after_the_dry_run() {
    // The one trace that holds the mutation: record on before the dry run, off for it, on again after the stop.
    let cases = vectors()["load"].as_array().unwrap();
    let case = cases
        .iter()
        .find(|c| c["name"].as_str().unwrap().starts_with("record on before"))
        .unwrap();
    let calls = rows(&case["calls"]);
    let at = |s: &str| calls.iter().position(|(_, w)| w == s).unwrap();
    assert!(at("set-record false") < at("transport play"));
    assert!(at("transport rewind") < at("set-record true"), "{calls:?}");
}

fn run_of(fx: &[Effect]) -> u64 {
    match fx {
        [Effect::AskLoaded { run: Some(r) }] => *r,
        other => panic!("one ask naming a run, got {other:?}"),
    }
}

#[test]
fn two_runs_on_two_links_answer_each_its_own_ask() {
    // F1: after a takeover both runs ask, and a swapped answer must not reach the other's wait.
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let doc = json!({ "masterVolume": -3 });
    let first = run_of(&l.warm_up(1, doc.clone(), clock.at(0)));
    let second = run_of(&l.warm_up(2, doc, clock.at(100)));
    assert_ne!(first, second);
    let fx = l.on(
        Event::Loaded {
            run: Some(second),
            held: Some(true),
        },
        clock.at(150),
    );
    assert_eq!(
        l.phase(),
        Some(Phase::Warming),
        "the newer run's answer warms it: {fx:?}"
    );
    let late = l.on(
        Event::Loaded {
            run: Some(first),
            held: Some(true),
        },
        clock.at(160),
    );
    assert!(
        late.is_empty(),
        "the replaced run ends without a word: {late:?}"
    );
    assert_eq!(l.phase(), Some(Phase::Warming));
}

#[test]
fn a_run_asked_and_never_answered_gives_up_on_its_clock() {
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let id = run_of(&l.warm_up(1, json!({}), clock.at(0)));
    let mut asks = 1;
    let mut said = vec![];
    let mut t = 0;
    while t <= 125_000 && l.phase().is_some() {
        t += 100;
        for e in l.on(Event::Tick(clock.at(t)), clock.at(t)) {
            match e {
                Effect::AskLoaded { run } => {
                    assert_eq!(run, Some(id));
                    asks += 1;
                }
                Effect::Log(m) => said.push(m),
                Effect::Restore => {}
                other => panic!("{other:?}"),
            }
        }
    }
    assert_eq!(
        l.phase(),
        None,
        "a load whose engine never answers lets the gate go"
    );
    assert!(said[0].contains("in time"), "{said:?}");
    // One ask per 2 s wait and 300 ms pause, not one for ever.
    assert!((50..=60).contains(&asks), "{asks} asks");
}

#[test]
fn an_engine_error_frame_during_the_dry_run_changes_nothing_but_a_failed_send_ends_it() {
    // F2: an unrelated `error` frame must not restore the master while the transport rolls.
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let id = run_of(&l.warm_up(1, json!({ "masterVolume": -3 }), clock.at(0)));
    l.on(
        Event::Loaded {
            run: Some(id),
            held: Some(true),
        },
        clock.at(10),
    );
    assert!(l.is_warming());
    assert!(
        l.on(Event::EngineError("unrelated".into()), clock.at(20))
            .is_empty()
    );
    assert!(l.is_warming());
    let fx = l.on(Event::SendFailed("boom".into()), clock.at(30));
    assert!(fx.contains(&Effect::Restore), "{fx:?}");
    assert!(!fx.contains(&Effect::Stop), "nothing was played: {fx:?}");
    assert_eq!(l.phase(), None);
}
