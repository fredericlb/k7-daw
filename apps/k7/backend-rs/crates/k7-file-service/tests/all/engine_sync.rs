//! The copy to the engine's machine (2223) against the TypeScript's traces: documents asked, jobs answered on a turned
//! clock, what is sent and what the poll shows. `settled` is only set by a job that answered.

use std::collections::{HashMap, VecDeque};

use k7_file_service::engine::sync::{Copies, Ctx, FileKey, Reply, SyncEffect, within};
use serde_json::{Value, json};

use crate::support::engine::{rows, vectors};

const TMP: &str = "/tmp/vec";

struct Front {
    copies: Copies,
    rows: Vec<(u64, String)>,
    lines: Vec<String>,
    now: u64,
    n: usize,
    url: Option<String>,
    files: HashMap<String, FileKey>,
    paths: Vec<String>,
    timers: Vec<(u64, Timer)>,
    asked: Vec<Value>,
    job: Value,
}

enum Timer {
    Progress(Value),
    End,
}

const SESSION: &str = "copy";

impl Front {
    fn ctx_run<R>(&mut self, f: impl FnOnce(&mut Copies, &Ctx) -> R) -> R {
        let files = self.files.clone();
        let stat = move |p: &str| files.get(p).copied();
        let url = self.url.clone();
        let ctx = Ctx {
            windows: false,
            url: url.as_deref(),
            stat: &stat,
            roots: None,
        };
        f(&mut self.copies, &ctx)
    }

    fn doc(&self, label: &str) -> Value {
        json!({ "label": label, "files": self.paths, "name": "inline source\nwith a line break", "relative": "a/b.wav" })
    }

    fn ask(&mut self, label: &str) {
        let doc = self.doc(label);
        let fx = self.ctx_run(|c, ctx| c.ask(SESSION, doc, vec![], ctx));
        self.apply(fx);
    }

    fn apply(&mut self, fx: Vec<SyncEffect>) {
        for e in fx {
            match e {
                SyncEffect::Send(d) => self
                    .rows
                    .push((self.now, format!("send {}", d["label"].as_str().unwrap()))),
                SyncEffect::Log(l) => self.lines.push(l),
                SyncEffect::Copy { url, files } => {
                    self.asked.push(json!({ "url": url, "files": files }));
                    if self.job.is_null() {
                        continue;
                    }
                    for p in self.job["progress"].as_array().cloned().unwrap_or_default() {
                        self.timers
                            .push((self.now + p["at"].as_u64().unwrap(), Timer::Progress(p)));
                    }
                    self.timers.push((
                        self.now + self.job["end"]["at"].as_u64().unwrap(),
                        Timer::End,
                    ));
                }
            }
        }
    }

    fn fire(&mut self, t: u64) {
        let due: Vec<_> = self
            .timers
            .iter()
            .filter(|(d, _)| *d <= t)
            .map(|(d, _)| *d)
            .collect();
        let mut keep = VecDeque::new();
        let mut fired = vec![];
        for (d, timer) in std::mem::take(&mut self.timers) {
            if due.contains(&d) && d <= t {
                fired.push((d, timer))
            } else {
                keep.push_back((d, timer))
            }
        }
        self.timers = keep.into();
        fired.sort_by_key(|(d, _)| *d);
        for (d, timer) in fired {
            self.now = d;
            match timer {
                Timer::Progress(p) => {
                    let mut frame = p.clone();
                    frame["type"] = json!("syncing");
                    self.copies.progress(SESSION, &frame);
                }
                Timer::End => {
                    let end = self.job["end"].clone();
                    let reply = if end["refuses"].as_bool() == Some(true) {
                        Reply::Failed("this machine does not do 'sync'".into())
                    } else if let Some(e) = end["error"].as_str() {
                        Reply::Failed(e.to_string())
                    } else {
                        Reply::Synced(
                            json!({ "type": "synced", "failed": end["failed"], "failures": end["failures"] }),
                        )
                    };
                    let fx = self.ctx_run(|c, ctx| c.finished(SESSION, reply, vec![], ctx));
                    self.apply(fx);
                }
            }
        }
        self.now = t;
    }
}

fn replay(i: usize, case: &Value) -> (Vec<(u64, String)>, Vec<String>, Vec<Value>) {
    let sc = &case["script"];
    let mut files = HashMap::new();
    let mut paths = vec![];
    for (name, size) in sc["files"].as_object().unwrap() {
        let p = format!("{TMP}/f/{name}");
        files.insert(
            p.clone(),
            FileKey {
                size: size.as_u64().unwrap(),
                mtime_ms: 1_000_000,
            },
        );
        paths.push(p);
    }
    let n = i + 1;
    let bound = |name: &str| format!("http://{name}/{n}/jobs");
    let mut f = Front {
        copies: Copies::new(),
        rows: vec![],
        lines: vec![],
        now: 0,
        n,
        url: sc["bound"].as_str().map(|_| bound("machine")),
        files,
        paths,
        timers: vec![],
        asked: vec![],
        job: sc["job"].clone(),
    };
    let mut events: VecDeque<(u64, String)> = sc["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e[0].as_u64().unwrap(), e[1].as_str().unwrap().to_string()))
        .collect();
    let until = sc["until"].as_u64().unwrap();
    let mut last = String::new();
    let mut t = 0;
    while t <= until {
        f.now = t;
        while events.front().is_some_and(|e| e.0 <= t) {
            let (_, op) = events.pop_front().unwrap();
            f.rows.push((t, format!("do {op}")));
            match op.as_str() {
                "ask" => f.ask("first"),
                "ask_second" => f.ask("second"),
                "retry" => f.copies.forget_failure(SESSION),
                "bind_other" => f.url = Some(bound("other")),
                "bind_none" => f.url = None,
                other => panic!("an op the vectors do not have: {other}"),
            }
        }
        let view = json!([
            f.copies.state(SESSION).map(|s| s.to_json()),
            f.copies.is_held_back(SESSION)
        ])
        .to_string();
        if view != last {
            f.rows.push((t, format!("view {view}")));
        }
        last = view;
        f.fire(t + 100);
        t += 100;
    }
    let _ = f.n;
    let asked = f.asked.iter().map(|a| json!({ "url": a["url"], "files": a["files"].as_array().unwrap().iter().map(|p| json!(p.as_str().unwrap().replace(TMP, "<tmp>"))).collect::<Vec<_>>() })).collect();
    (f.rows, f.lines, asked)
}

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

fn named(rows: Vec<(u64, String)>) -> Vec<(u64, String)> {
    rows.into_iter()
        .map(|(t, w)| (t, w.replace("<tmp>", TMP)))
        .collect()
}

#[test]
fn every_trace_of_the_copy_is_replayed() {
    let cases = vectors()["copy"].as_array().unwrap();
    assert!(cases.len() >= 9);
    for (i, case) in cases.iter().enumerate() {
        let name = case["name"].as_str().unwrap();
        let (got, lines, asked) = replay(i, case);
        assert_eq!(
            normal(&got),
            normal(&named(rows(&case["rows"]))),
            "{name}: rows"
        );
        let want_lines: Vec<&str> = case["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(lines, want_lines, "{name}: lines");
        assert_eq!(Value::Array(asked), case["asked"], "{name}: the jobs asked");
    }
}

fn one_file(_: &str) -> Option<FileKey> {
    Some(FileKey {
        size: 5,
        mtime_ms: 7,
    })
}

#[test]
fn a_copy_for_a_machine_the_binding_left_settles_nothing_of_the_new_one() {
    // F6: a copy to A, the binding moves to B, the session asks again mid-copy; A's answer must not settle B's files.
    let mut c = Copies::new();
    let doc = json!({ "f": "/ws/a.wav" });
    let on = |url| Ctx {
        windows: false,
        url: Some(url),
        stat: &one_file,
        roots: None,
    };
    let fx = c.ask("s", doc.clone(), vec![], &on("ws://a/jobs"));
    assert!(matches!(fx.as_slice(), [SyncEffect::Copy { url, .. }] if url == "ws://a/jobs"));
    assert!(
        c.ask("s", doc, vec![], &on("ws://b/jobs")).is_empty(),
        "the run in flight takes the document"
    );
    let ok = Reply::Synced(json!({ "type": "synced", "failed": 0 }));
    let fx = c.finished("s", ok, vec![], &on("ws://b/jobs"));
    assert!(
        matches!(fx.as_slice(), [SyncEffect::Copy { url, files }] if url == "ws://b/jobs" && files == &["/ws/a.wav"]),
        "the new machine is copied to, not sent a document without its files: {fx:?}"
    );
}

#[test]
fn the_roots_compare_the_way_a_windows_host_spells_a_path() {
    // F5: separators and a drive's case are one path, and nothing else is inside.
    let roots = vec!["C:/a".to_string(), "\\\\srv\\share".to_string()];
    for inside in [
        "C:\\a\\x",
        "C:/a/x",
        "c:/a/x",
        "C:/a",
        "\\\\srv\\share\\f",
        "//srv/share/f",
    ] {
        assert!(within(inside, &roots, true), "{inside}");
    }
    for outside in [
        "C:/ab",
        "C:",
        "/a",
        "C:/a/../b",
        "D:/a/x",
        "\\\\srv\\other\\f",
    ] {
        assert!(!within(outside, &roots, true), "{outside}");
    }
    // On another host a backslash and a lower-case drive are not the root's spelling.
    assert!(!within("c:/a/x", &roots, false));
    assert!(!within("C:\\a\\x", &roots, false));
    assert!(within("C:/a/x", &roots, false));
}
