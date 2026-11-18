//! The scan wait (2222) against the TypeScript's traces: a link's frames and a turned clock in, the holds, the resends and
//! the poll's view out. The 60 s stall and the 10 minute bound are driven on the clock, not waited for.

use std::collections::VecDeque;

use k7_file_service::engine::load::Load;
use k7_file_service::engine::scan::Scan;
use k7_file_service::engine::wire::decode_scan;
use k7_file_service::engine::{Effect, Event, ScanStatus};
use serde_json::{Value, json};

use crate::support::engine::{Clock, rows, vectors};

struct Front {
    clock: Clock,
    now: u64,
    load: Load,
    scan: Scan,
    rows: Vec<(u64, String)>,
    lines: Vec<String>,
    link_scan: Option<ScanStatus>,
    written: Option<Value>,
    later: Vec<(u64, Value)>,
    catalogue_after: Option<u64>,
    external: bool,
}

const LINK: u64 = 1;

fn status(v: &Value) -> ScanStatus {
    let mut f = v.clone();
    f["type"] = json!("plugin-scan");
    decode_scan(&f).unwrap()
}

impl Front {
    fn row(&mut self, w: String) {
        self.rows.push((self.now, w));
    }

    fn run(&mut self, fx: Vec<Effect>) {
        for e in fx {
            match e {
                Effect::AskScan => self.row("ask plugin-scan".into()),
                Effect::AskCatalogue => {
                    self.row("ask list-plugins".into());
                    if let Some(after) = self.catalogue_after {
                        self.later.push((self.now + after, json!([{ "name": "Surge", "format": "VST3", "manufacturer": "m", "role": "effect" }])));
                    }
                }
                Effect::WriteCatalogue(list) => self.written = Some(list),
                Effect::Resend => self.row("resend".into()),
                Effect::Log(l) => self.lines.push(l),
                other => panic!("a scan never says {other:?}"),
            }
        }
    }

    fn holds(&mut self, project: &str) -> bool {
        let (held, fx) = self.scan.holds(
            &mut self.load,
            (project, "r1"),
            LINK,
            self.link_scan.as_ref(),
            || self.external || project == "q",
            self.clock.at(self.now),
        );
        self.run(fx);
        held
    }

    fn feed(&mut self, e: Event) {
        let fx = self.scan.on(&mut self.load, e, self.clock.at(self.now));
        self.run(fx);
    }

    fn view(&self) -> String {
        let phase = self.load.phase().map_or(Value::Null, |p| json!(p.as_str()));
        let scan = self
            .scan
            .view(self.clock.at(self.now))
            .map_or(Value::Null, |v| v.to_json());
        json!([phase, scan]).to_string()
    }
}

fn replay(case: &Value) -> (Vec<(u64, String)>, Vec<String>, Option<Value>) {
    let sc = &case["script"];
    let mut f = Front {
        clock: Clock::new(),
        now: 0,
        load: Load::new(),
        scan: Scan::new(),
        rows: vec![],
        lines: vec![],
        link_scan: sc["first"].as_object().map(|_| status(&sc["first"])),
        written: None,
        later: vec![],
        catalogue_after: sc["catalogueAfter"].as_u64(),
        external: sc["external"].as_bool() == Some(true),
    };
    let fx = f.load.begin();
    assert!(fx.is_empty());
    let held = f.holds("p");
    f.row(format!("holds {held}"));
    let mut steps: VecDeque<Value> = {
        let mut s: Vec<Value> = sc["steps"].as_array().unwrap().clone();
        s.sort_by_key(|x| x[0].as_u64().unwrap());
        s.into()
    };
    let mut others: VecDeque<u64> = sc["other"]
        .as_array()
        .map(|a| a.iter().map(|x| x.as_u64().unwrap()).collect())
        .unwrap_or_default();
    let until = sc["until"].as_u64().unwrap();
    let mut last = String::new();
    let mut t = 0;
    while t <= until {
        f.now = t;
        while steps.front().is_some_and(|s| s[0].as_u64().unwrap() <= t) {
            let s = steps.pop_front().unwrap();
            match s[1].as_str().unwrap() {
                "scan" => {
                    let st = status(&s[2]);
                    f.link_scan = Some(st.clone());
                    f.feed(Event::Scan(st));
                }
                "plugins" => f.feed(Event::Plugins(s[2].clone())),
                "close" => f.feed(Event::Closed { link: LINK }),
                "error" => f.feed(Event::EngineError(s[2].as_str().unwrap().to_string())),
                other => panic!("a step the vectors do not have: {other}"),
            }
        }
        while others.front().is_some_and(|o| *o <= t) {
            others.pop_front();
            let held = f.holds("q");
            f.row(format!("holds other {held}"));
        }
        let view = f.view();
        if view != last {
            f.row(format!("view {view}"));
        }
        last = view;
        // The window to t + 100: timers fire at their due time, so the clock is turned before the tick.
        f.now = t + 100;
        let due: Vec<Value> = {
            let (now, rest): (Vec<_>, Vec<_>) = f.later.drain(..).partition(|(d, _)| *d <= f.now);
            f.later = rest;
            now.into_iter().map(|(_, v)| v).collect()
        };
        for v in due {
            f.feed(Event::Plugins(v));
        }
        f.feed(Event::Tick(f.clock.at(f.now)));
        t += 100;
    }
    f.now = until + 100;
    let again = f.holds("p");
    f.row(format!("again {again}"));
    (f.rows, f.lines, f.written)
}

/// The views are JSON, compared as JSON: key order is no business of the poll's reader.
fn normal(rows: &[(u64, String)]) -> Vec<(u64, Value)> {
    rows.iter()
        .map(|(t, w)| {
            (
                *t,
                w.strip_prefix("view ")
                    .map_or(json!(w), |v| serde_json::from_str(v).unwrap()),
            )
        })
        .collect()
}

#[test]
fn every_trace_of_the_scan_is_replayed() {
    let cases = vectors()["scan"].as_array().unwrap();
    assert!(cases.len() >= 12);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let (got, lines, written) = replay(case);
        let want = rows(&case["rows"]);
        assert_eq!(normal(&got), normal(&want), "{name}: rows");
        let want_lines: Vec<&str> = case["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(lines, want_lines, "{name}: lines");
        // The catalogue written is the plugins the engine named, in the copy's shape (the front writes it).
        match case["catalogue"].as_str() {
            None => assert!(written.is_none(), "{name}: nothing is written"),
            Some(text) => {
                let copy: Value = serde_json::from_str(text).unwrap();
                assert_eq!(
                    written,
                    Some(copy["entries"][0]["plugins"].clone()),
                    "{name}: catalogue"
                );
            }
        }
    }
}

#[test]
fn the_ten_minute_bound_is_a_count_not_a_wait() {
    let cases = vectors()["scan"].as_array().unwrap();
    let case = cases
        .iter()
        .find(|c| c["name"].as_str().unwrap().contains("ten minutes"))
        .unwrap();
    let want = rows(&case["rows"]);
    assert!(
        want.iter()
            .any(|(t, w)| w == "resend" && (600_000..=601_000).contains(t)),
        "{want:?}"
    );
}
