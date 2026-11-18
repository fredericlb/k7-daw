//! The poll's reconcile (2225) against the TypeScript's traces: a poll at a time against an engine that answers from a script.
//! The front here dials a link per `ensureEngine`, so "redialled" counts the sockets built, the first included.

use std::collections::VecDeque;

use k7_file_service::engine::reconcile::{LinkState, Reconcile};
use k7_file_service::engine::{Effect, Event, Phase};
use serde_json::Value;

use crate::support::engine::{Clock, rows, same_rows, vectors};

struct Front {
    clock: Clock,
    now: u64,
    r: Reconcile,
    link: Option<LinkState>,
    built: u32,
    asked: u32,
    resent: u32,
    lines: Vec<String>,
    never_opens: bool,
    answer: String,
    micro: VecDeque<Option<bool>>,
}

impl Front {
    fn dial(&mut self) {
        self.built += 1;
        self.link = Some(LinkState::Dialling);
    }

    fn run(&mut self, fx: Vec<Effect>) {
        for e in fx {
            match e {
                Effect::AskLoaded { run: None } => {
                    self.asked += 1;
                    match self.answer.as_str() {
                        "loaded" => self.micro.push_back(Some(true)),
                        "empty" => self.micro.push_back(Some(false)),
                        _ => {}
                    }
                }
                Effect::Resend => self.resent += 1,
                Effect::Redial => self.dial(),
                Effect::Log(l) => self.lines.push(l),
                other => panic!("a reconcile never says {other:?}"),
            }
        }
    }

    fn feed(&mut self, e: Event) {
        let fx = self.r.on(e);
        self.run(fx);
    }
}

fn replay(case: &Value) -> (Vec<(u64, String)>, Vec<String>) {
    let sc = &case["script"];
    let scanning = sc["scanning"].as_bool() == Some(true);
    let mut f = Front {
        clock: Clock::new(),
        now: 0,
        r: Reconcile::new("k7"),
        link: None,
        built: 0,
        asked: 0,
        resent: 0,
        lines: vec![],
        never_opens: sc["neverOpens"].as_bool() == Some(true),
        answer: String::new(),
        micro: VecDeque::new(),
    };
    let phase = scanning.then_some(Phase::Scanning);
    let mut polls: VecDeque<(u64, String)> = sc["polls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["at"].as_u64().unwrap(),
                p["answer"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let until = sc["until"].as_u64().unwrap();
    let mut out: Vec<(u64, String)> = vec![];
    // The poll whose row is still to be said: its counters at the start, and what it answered.
    let mut open: Option<(String, u32, u32, u32)> = None;
    let mut t = 0;
    while t <= until {
        f.now = t;
        // A socket opens on a later turn than its dial.
        if f.link == Some(LinkState::Dialling) && !f.never_opens {
            f.link = Some(LinkState::Ready);
        }
        while polls.front().is_some_and(|p| p.0 <= t) {
            let (_, answer) = polls.pop_front().unwrap();
            f.answer = answer.clone();
            let (built, asked) = (f.built, f.asked);
            f.resent = 0;
            if f.r.begin(f.clock.at(t), false, phase) {
                if f.link.is_none() {
                    f.dial();
                }
                let fx =
                    f.r.ask(f.clock.at(t), "/rec/song.k7", f.link.unwrap(), phase);
                f.run(fx);
            }
            open = Some((answer, built, asked, f.resent));
        }
        // Microtasks of this instant, then the poll's row if it is over.
        while let Some(v) = f.micro.pop_front() {
            f.feed(Event::Loaded { run: None, held: v });
        }
        close(&mut f, &mut open, &mut out, t);
        f.now = t + 100;
        f.feed(Event::Tick(f.clock.at(t + 100)));
        close(&mut f, &mut open, &mut out, t + 100);
        t += 100;
    }
    (out, f.lines)
}

/// A poll is over when the machine no longer waits on its answer (asking is private, so the counters decide).
fn close(
    f: &mut Front,
    open: &mut Option<(String, u32, u32, u32)>,
    out: &mut Vec<(u64, String)>,
    at: u64,
) {
    if let Some((answer, built, asked, _)) = open.as_ref()
        && !f.waiting()
    {
        let row = format!(
            "poll {answer}: asked {}, resent {}, redialled {}, resends {}",
            f.asked - asked,
            f.resent,
            f.built - built,
            f.r.resends
        );
        out.push((at, row));
        *open = None;
    }
}

impl Front {
    /// Whether an ask is outstanding: asked more than answered.
    fn waiting(&self) -> bool {
        self.r.asking()
    }
}

#[test]
fn every_trace_of_the_reconcile_is_replayed() {
    let cases = vectors()["reconcile"].as_array().unwrap();
    assert!(cases.len() >= 7);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let (got, lines) = replay(case);
        same_rows(name, &got, &rows(&case["rows"]));
        let want: Vec<&str> = case["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(lines, want, "{name}: lines");
    }
}
