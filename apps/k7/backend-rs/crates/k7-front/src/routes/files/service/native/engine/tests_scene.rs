//! Helpers of the engine link's frame replays: a scenario's steps, and the trace read back with a dry run's window counted.
//! What a reader gets wrong: a step waits on a landmark (the load done, a frame recorded), never on quiet.

use std::time::{Duration, Instant};

use k7_file_service::engine::docs::TakeArm;
use serde_json::{Value, json};

use super::tests_real::Account;

/// A render's fingerprint left out of its name: the two sides name one render differently (decision 343).
fn render_named(s: &str) -> String {
    match s.split_once("/.renders/") {
        Some((head, name)) if name.ends_with(".wav") => {
            let module = name.rsplit_once('-').map_or(name, |(m, _)| m);
            format!("{head}/.renders/{module}-<fingerprint>.wav")
        }
        _ => s.to_owned(),
    }
}

/// A frame as the comparison reads it: the session left out (each side names its own), every number a float.
fn norm(v: &Value, dir: &str) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64()),
        // A take's clip is named by its stamp, which each side makes for itself.
        Value::String(s) if s.starts_with("gtr_") => json!("<take>"),
        Value::String(s) if s.contains("clips/gtr_") => json!("<take file>"),
        Value::String(s) => json!(render_named(
            &crate::routes::files::service::native::fold::fold_dir(s, dir)
        )),
        Value::Array(a) => Value::Array(a.iter().map(|x| norm(x, dir)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| k.as_str() != "session")
                .map(|(k, v)| (k.clone(), norm(v, dir)))
                .collect(),
        ),
        v => v.clone(),
    }
}

pub(super) fn frames(a: &Account) -> Vec<Value> {
    let mut got = a.fake.got();
    got.sort_by_key(|g| g.at);
    let dir = a.dir.display().to_string();
    got.iter()
        .filter(|g| g.frame["type"] != "list-sessions")
        .map(|g| norm(&g.frame, &dir))
        .collect()
}

/// A document at the silenced master level: the first frame of a dry run's window (2270).
fn silenced(f: &Value) -> bool {
    f["type"] == "project" && f["doc"]["masterVolume"] == json!(-100.0)
}

/// A frame only a dry run's window drops: the record switch and the transport it plays with.
fn window_frame(f: &Value) -> bool {
    match f["type"].as_str() {
        Some("record-enable") => true,
        Some("transport") => matches!(f["action"].as_str(), Some("play" | "stop" | "rewind")),
        _ => false,
    }
}

/// `compared`, once the trace is quiet and no window is left open: a trace read while a dry run's restore or its first
/// document is still on its way would fail it. A pending lead-in is quiet for longer, since a late dry run ends so.
pub(super) async fn settled(a: &Account) -> Compared {
    let end = Instant::now() + Duration::from_secs(60);
    let (mut seen, mut changed) = (frames(a).len(), Instant::now());
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let now = frames(a).len();
        if now != seen {
            (seen, changed) = (now, Instant::now());
        }
        let c = compared(a);
        let quiet = Duration::from_millis(if c.trailing > 0 { 3000 } else { 400 });
        if (!c.open_at_end && changed.elapsed() >= quiet) || Instant::now() > end {
            return c;
        }
    }
}

/// What one dry run's window dropped: the record switch and the transport it plays with, and the dry runs it holds.
#[derive(Default, Clone, Debug, PartialEq)]
pub(super) struct Window {
    pub dries: usize,
    pub record: usize,
    pub play: usize,
    pub stop: usize,
    pub rewind: usize,
}

/// A trace read for the comparison: the kept frames, and what was dropped or collapsed to get them.
pub(super) struct Compared {
    pub frames: Vec<Value>,
    /// Documents equal to the one kept before them, inside a window or in the lead-in to one (2270).
    pub collapsed: usize,
    /// Documents equal to the one kept before them anywhere else: only a scenario's own resend may cause one.
    pub strays: usize,
    pub dries: usize,
    pub windows: Vec<Window>,
    /// A window that never saw its rewind and its real document: its drops would hide every later transport.
    pub open_at_end: bool,
    /// Documents equal to the last kept one at the very end: a lead-in whose dry run has not begun yet, or a resend.
    pub trailing: usize,
}

/// The frames without the dry runs', counted. A window runs from a silenced document to the document at the real level
/// that follows its rewind (a stray one before the rewind is a resend, kept). Only inside a window are the
/// silenced documents, record and transport frames dropped; a collapse is inside one or in the lead-in to one.
pub(super) fn compared(a: &Account) -> Compared {
    let mut c = Compared {
        frames: vec![],
        collapsed: 0,
        strays: 0,
        dries: 0,
        windows: vec![],
        open_at_end: false,
        trailing: 0,
    };
    let (mut now, mut open, mut rewound, mut lead) = (Window::default(), false, false, 0);
    for f in frames(a) {
        if silenced(&f) {
            c.dries += 1;
            now.dries += 1;
            (open, c.collapsed) = (true, c.collapsed + lead);
            lead = 0;
            continue;
        }
        if open && window_frame(&f) {
            match (f["type"].as_str(), f["action"].as_str()) {
                (Some("record-enable"), _) => now.record += 1,
                (_, Some("play")) => now.play += 1,
                (_, Some("stop")) => now.stop += 1,
                _ => {
                    now.rewind += 1;
                    rewound = true;
                }
            }
            continue;
        }
        if f["type"] == "project" {
            let inside = open;
            if open && rewound {
                c.windows.push(std::mem::take(&mut now));
                (open, rewound) = (false, false);
            }
            if c.frames.last() == Some(&f) {
                if inside {
                    c.collapsed += 1;
                } else {
                    lead += 1;
                }
                continue;
            }
        }
        (c.strays, lead) = (c.strays + lead, 0);
        c.frames.push(f);
    }
    c.strays += lead;
    c.trailing = lead;
    c.open_at_end = open;
    if open {
        c.windows.push(now);
    }
    c
}

/// One frame in a line: its type, action and, for a document, its tempo and master level.
pub(super) fn brief(f: &Value) -> String {
    let t = f["type"].as_str().unwrap_or("?");
    match t {
        "project" => format!(
            "project(tempo {}, master {})",
            f["doc"]["tempo"], f["doc"]["masterVolume"]
        ),
        _ => match f["action"].as_str() {
            Some(a) => format!("{t} {a}"),
            None => t.to_string(),
        },
    }
}

fn side(a: &Account) -> &'static str {
    if a.engine.is_some() { "on" } else { "off" }
}

/// Wait until a frame the engine read satisfies `want`, from index `from` of the frames; the index it was found at.
pub(super) async fn until_frame(
    a: &Account,
    from: usize,
    what: &str,
    want: impl Fn(&Value) -> bool,
) -> usize {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let all = frames(a);
        if let Some(i) = all.iter().skip(from).position(&want) {
            return from + i;
        }
        assert!(
            Instant::now() < end,
            "{}: timed out waiting for {what}: {all:?}",
            side(a)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A file written in the project and pushed, as a save is.
pub(super) async fn write_wait(a: &Account, path: &str, content: &str) {
    std::fs::write(std::path::Path::new(&a.ws).join(path), content).unwrap();
    a.engine.as_ref().unwrap().push_active(&a.ws).await;
}

/// What a verb sends the engine, as the verbs build it: the frame, or the take's arm held on the link.
pub(super) async fn verb(a: &Account, line: &str) {
    let s = a
        .engine
        .as_ref()
        .expect("the link")
        .link
        .session(Some(&a.ws));
    match line {
        "transport arm gtr on" => {
            let _ = s.frame(json!({ "type": "arm", "track": "gtr", "armed": true }));
        }
        "transport arm gtr off" => {
            let _ = s.frame(json!({ "type": "arm", "track": "gtr", "armed": false }));
        }
        "system plugins list" => {
            let _ = s.frame(json!({ "type": "list-plugins" }));
        }
        "event take arm gtr 1:1 1:1 3:1" => {
            // As the `takeArm` route does: the arm set on the session, then the served document sent again.
            s.set_take_arm(Some(TakeArm {
                track: "gtr".into(),
                event_beats: 0.0,
                from: 0.0,
                to: Some(8.0),
            }));
            s.enqueue(|s| async move { s.resend_served().await });
        }
        other => panic!("no native step for the verb {other}"),
    }
}

/// The load phase of the open project, or none once it is playable.
fn phase(a: &Account) -> Option<String> {
    let e = a.engine.as_ref().expect("the front's side");
    e.link
        .session(Some(&a.ws))
        .load_phase()
        .map(|p| p.as_str().to_string())
}

/// Playable: the session serves the project and the poll's `engineLoad` is null; the time it took is printed.
pub(super) async fn playable(a: &Account, name: &str, since: Instant, project: &str) {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let served = a
            .engine
            .as_ref()
            .is_none_or(|e| e.link.session(Some(&a.ws)).serving().as_deref() == Some(project));
        if served && phase(a).is_none() {
            break;
        }
        assert!(Instant::now() < end, "{name} {}: never playable", side(a));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    eprintln!(
        "frames: {name} {}: playable in {} ms",
        side(a),
        since.elapsed().as_millis()
    );
}

/// The engine socket dialled and open before the project is: a first open then sends its document once on either link (2270).
async fn dial(a: &Account) {
    let link = &a.engine.as_ref().expect("the link").link;
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        // The probe is sent only on an open socket; the answer is offline otherwise.
        let s = link.session(Some(&a.ws));
        let sent = s.dial_ready() && s.frame(json!({ "type": "list-plugins" }));
        while sent && Instant::now() < end && !frames(a).iter().any(is_frame("list-plugins")) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if frames(a).iter().any(is_frame("list-plugins")) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{}: the socket never opened", side(a));
}

/// `open` and the engine start 1g's route makes, ended on the landmark.
pub(super) async fn open(a: &Account, name: &str, path: &str) {
    a.open(path);
    dial(a).await;
    let since = Instant::now();
    a.start().await;
    playable(a, name, since, &format!("{}/{path}", a.ws)).await;
}

pub(super) type Scenario =
    for<'a> fn(
        &'a Account,
        &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>;

pub(super) fn is_frame(kind: &'static str) -> impl Fn(&Value) -> bool {
    move |f| f["type"] == kind
}

/// An open, a verb's transport, a save inside the project and its push, a transport after it.
pub(super) fn save_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let before = frames(a).len();
        verb(a, "transport arm gtr on").await;
        until_frame(a, before, "the verb's arm", is_frame("arm")).await;
        let seen = frames(a).len();
        write_wait(a, "p.k7/project.k7x", "project({ tempo: 90 })\n").await;
        until_frame(a, seen, "the save's document", |f| {
            f["type"] == "project" && f["doc"]["tempo"] == json!(90.0)
        })
        .await;
        let seen = frames(a).len();
        verb(a, "transport arm gtr off").await;
        until_frame(a, seen, "the verb's disarm", |f| {
            f["type"] == "arm" && f["armed"] == json!(false)
        })
        .await;
    })
}

/// An open, the project let go (`letGo` on both sides), and opened again.
pub(super) fn unload_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let before = frames(a).len();
        verb(a, "transport arm gtr on").await;
        until_frame(a, before, "the arm", is_frame("arm")).await;
        let seen = frames(a).len();
        a.unload().await;
        until_frame(a, seen, "the drop", is_frame("drop-session")).await;
        open(a, name, "p.k7").await;
    })
}

/// A switch to a second project of the workspace.
pub(super) fn switch_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let q = std::path::Path::new(&a.ws).join("q.k7");
        std::fs::create_dir_all(&q).unwrap();
        // A project the song crate compiles: the native compile refuses one with no graph, no modules and no chains.
        std::fs::create_dir_all(q.join("nodes")).unwrap();
        std::fs::write(q.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
        std::fs::write(q.join("graph.k7x"), "modules({ gtr: [\"MIX\"] })\n").unwrap();
        std::fs::write(q.join("project.k7x"), "project({ tempo: 80 })\n").unwrap();
        let seen = frames(a).len();
        open(a, name, "q.k7").await;
        until_frame(a, seen, "the second document", |f| {
            f["type"] == "project" && f["doc"]["tempo"] == json!(80.0)
        })
        .await;
    })
}
