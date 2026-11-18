//! jobs_freezer_traces.rs — the TypeScript's freezer traces (`jobs.json`, 2303) replayed against the machine, effect for effect (2312 to 2316).
//! A reader gets wrong that `advance` is a no-op here: the 200 ms poll of `renderAllNow` is 2315's Notify, so a queued
//! `renderAll` starts when the pass ends: the replay compares the effects step by step, a release or an advance merged into the step before.

use crate::jobs_freezer::freeze;
use crate::support::jobs::{section, vectors};
use k7_file_service::jobs::freezer::Freezer;
use k7_file_service::jobs::{FreezeEffect as Fx, FreezeEvent as Ev};
use k7_file_service::paths::resolve_all;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

struct Replay {
    freezer: Freezer,
    base: Instant,
    tables: HashMap<String, Vec<Value>>,
    made: HashSet<String>,
    renders: HashMap<String, String>,
    second_read: HashMap<String, bool>,
    held: Vec<(String, String)>,
    hold: bool,
    fail_compile: Option<String>,
    told: HashMap<String, Vec<String>>,
    log: Vec<String>,
}

impl Replay {
    fn at(&self, ms: u64) -> Instant {
        self.base + Duration::from_millis(ms)
    }

    fn compile(&self, dir: &str) -> Ev {
        if let Some(why) = &self.fail_compile {
            return Ev::CompileFailed(why.clone());
        }
        let key = resolve_all(&[dir]);
        let freezes = self.tables.get(&key).map(Vec::as_slice).unwrap_or_default();
        let freezes = freezes
            .iter()
            .map(|f| {
                let module = f["module"].as_str().unwrap();
                let plugins: Vec<&str> = f["plugins"].as_array().map_or(vec![module], |p| {
                    p.iter().filter_map(Value::as_str).collect()
                });
                let mut made = freeze(
                    module,
                    false,
                    f["hosted"].as_bool().unwrap(),
                    &plugins,
                    &key,
                );
                made.ready = f["ready"].as_bool().unwrap() || self.made.contains(&made.render);
                made
            })
            .collect();
        Ev::Compiled {
            freezes,
            playing: self.second_read.get(&key).copied().unwrap_or(false),
        }
    }

    fn answer(&mut self, dir: &str, module: &str) -> Ev {
        let key = resolve_all(&[dir]);
        let render = freeze(module, false, true, &[], &key).render;
        let how = self
            .renders
            .get(module)
            .cloned()
            .unwrap_or_else(|| "ok".into());
        let error = how.strip_prefix("fail:").map(str::to_owned);
        if how == "ok" {
            self.made.insert(render.clone());
        }
        Ev::Rendered {
            module: module.to_owned(),
            made: self.made.contains(&render),
            error,
        }
    }

    fn feed(&mut self, dir: &str, first: Ev) {
        let mut queue = vec![first];
        while let Some(ev) = queue.pop() {
            for fx in self.freezer.on(dir, ev) {
                match fx {
                    Fx::Compile => {
                        self.log.push("compile".into());
                        queue.push(self.compile(dir));
                    }
                    Fx::Render(f) => {
                        self.log.push(format!("render {}", f.module));
                        if self.hold {
                            self.held.push((dir.to_owned(), f.module));
                        } else {
                            queue.push(self.answer(dir, &f.module));
                        }
                    }
                    Fx::Reload => self.log.push("reload".into()),
                    Fx::Log(l) => self.log.push(format!("log {l}")),
                    Fx::Done(v) => self
                        .log
                        .push(format!("tick -> {}", serde_json::to_string(&v).unwrap())),
                    Fx::Progress {
                        done,
                        total,
                        module,
                    } => {
                        self.log.push(format!(
                            "progress {done} {total} {}",
                            serde_json::to_string(&module).unwrap()
                        ));
                    }
                    Fx::Impossible(list) => {
                        let rows: Vec<Value> = list
                            .iter()
                            .map(|(m, p)| json!({ "module": m, "plugins": p }))
                            .collect();
                        self.log.push(format!(
                            "renderAll -> {}",
                            serde_json::to_string(&rows).unwrap()
                        ));
                    }
                    Fx::RenderAllFailed(why) => self.log.push(format!("renderAll threw {why}")),
                    Fx::SetThawed(list) => {
                        self.told.insert(resolve_all(&[dir]), list);
                    }
                }
            }
        }
    }
}

fn table(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn replay(t: &Value) -> Vec<String> {
    let primary = t["dir"].as_str().unwrap().to_owned();
    for (spelling, key) in t["keys"].as_object().unwrap() {
        assert_eq!(
            &resolve_all(&[spelling]),
            key.as_str().unwrap(),
            "the key of {spelling}"
        );
    }
    let mut r = Replay {
        freezer: Freezer::new(),
        base: Instant::now(),
        tables: HashMap::new(),
        made: HashSet::new(),
        renders: HashMap::new(),
        second_read: HashMap::new(),
        held: Vec::new(),
        hold: false,
        fail_compile: None,
        told: HashMap::new(),
        log: Vec::new(),
    };
    let start: Vec<String> = t["start_thawed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_owned())
        .collect();
    r.freezer.set_thawed(&primary, &start);
    let dirs: Vec<String> = t["keys"].as_object().unwrap().keys().cloned().collect();
    let mut before: HashMap<String, Vec<String>> = dirs
        .iter()
        .map(|d| (d.clone(), r.freezer.thawed(d)))
        .collect();
    for (i, s) in t["steps"].as_array().unwrap().iter().enumerate() {
        r.told.clear();
        let op = s["op"].as_str().unwrap();
        r.log.push(format!("step {i} {op}"));
        let dir = s["dir"].as_str().unwrap_or(&primary).to_owned();
        let now = r.at(s["at"].as_u64().unwrap());
        if let Some(rs) = s.get("renders").and_then(Value::as_object) {
            r.renders = rs
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect();
        }
        match op {
            "tick" => {
                if s.get("compile").is_some() {
                    r.tables.insert(resolve_all(&[&dir]), table(&s["compile"]));
                }
                r.fail_compile = s.get("fail").and_then(Value::as_str).map(str::to_owned);
                r.hold = s.get("hold").and_then(Value::as_bool).unwrap_or(false);
                // The TypeScript reads `playing()` once, and again after the compile: the first and second of the list, or its only one.
                let seq: Vec<bool> = s
                    .get("playing")
                    .and_then(Value::as_array)
                    .map_or(vec![false], |p| {
                        p.iter().map(|b| b.as_bool().unwrap()).collect()
                    });
                r.second_read
                    .insert(resolve_all(&[&dir]), *seq.get(1).unwrap_or(&seq[0]));
                let rev = s["rev"].as_str().unwrap().to_owned();
                r.feed(
                    &dir,
                    Ev::Tick {
                        rev,
                        playing: seq[0],
                        now,
                    },
                );
            }
            "thaw" => r.feed(
                &dir,
                Ev::Thaw {
                    module: s["module"].as_str().unwrap().to_owned(),
                    on: s["on"].as_bool().unwrap(),
                    now,
                },
            ),
            "renderAll" => {
                r.tables.insert(resolve_all(&[&dir]), table(&s["compile"]));
                r.fail_compile = s.get("fail").and_then(Value::as_str).map(str::to_owned);
                r.hold = false;
                r.feed(&dir, Ev::RenderAll);
            }
            "release" => {
                r.hold = false;
                for (d, m) in std::mem::take(&mut r.held) {
                    let ev = r.answer(&d, &m);
                    r.feed(&d, ev);
                }
            }
            "advance" => {}
            other => panic!("an op the replay does not know: {other}"),
        }
        for d in &dirs {
            // A change of the thawed list reaches the freezer as the last `SetThawed` of the step.
            let now_list = r.freezer.thawed(d);
            if before[d] != now_list {
                assert_eq!(
                    r.told.get(&resolve_all(&[d])),
                    Some(&now_list),
                    "{}: step {i} changed the thawed list without telling it",
                    t["name"]
                );
            }
            before.insert(d.clone(), now_list);
            let list = serde_json::to_string(&r.freezer.thawed(d)).unwrap();
            r.log.push(format!(
                "thawed{} {list}",
                if *d == primary {
                    String::new()
                } else {
                    format!(" {d}")
                }
            ));
        }
    }
    r.log
}

/// The effects of each step, a `release` or an `advance` merged into the step before it (2315 moves the queued renderAll there).
fn effects_by_step(log: &[String]) -> Vec<Vec<&str>> {
    let mut steps: Vec<Vec<&str>> = Vec::new();
    for l in log.iter().map(String::as_str) {
        if let Some(marker) = l.strip_prefix("step ") {
            let op = marker.split(' ').nth(1).unwrap_or("");
            if !(matches!(op, "release" | "advance") && !steps.is_empty()) {
                steps.push(Vec::new());
            }
        } else if !l.starts_with("thawed") {
            steps.last_mut().expect("a step marker first").push(l);
        }
    }
    steps
}

fn thawed_of(log: &[String]) -> Vec<&str> {
    log.iter()
        .map(String::as_str)
        .filter(|l| l.starts_with("thawed"))
        .collect()
}

#[test]
fn every_trace_of_the_typescript_freezer_is_replayed_effect_for_effect() {
    let traces = section(&vectors(), "freezer");
    assert!(traces.len() >= 18, "{} traces", traces.len());
    for t in &traces {
        let got = replay(t);
        let want: Vec<String> = t["log"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_owned())
            .collect();
        let name = t["name"].as_str().unwrap();
        assert_eq!(
            effects_by_step(&got),
            effects_by_step(&want),
            "{name}: the effects, step by step"
        );
        assert_eq!(
            thawed_of(&got),
            thawed_of(&want),
            "{name}: the thawed list after each step"
        );
    }
}
