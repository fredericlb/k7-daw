//! `POST /verb` through a real account and `dispatch`: a verb run natively on a fixture project on the disk, its writes one journal step
//! that undo lays back, a refusal that writes nothing and makes no step, and a project-less verb that is never a step.

use std::path::Path;

use serde_json::{Value, json};

use super::Tag;
use super::docs::Docs;
use super::tests_account::{call, start, text};
use super::tests_docs::code_root;
use super::tests_journal::Bed;
use crate::routes::files::service::routes::{self, H};

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A bed whose open project is `e2e/fixtures/clip-lane.k7`, with the fixtures' `_modules` beside it.
async fn clip_lane(name: &str) -> Bed {
    let b = Bed::new(name).await;
    let fixtures = code_root().join("e2e/fixtures");
    copy_tree(&fixtures.join("clip-lane.k7"), Path::new(&b.project));
    copy_tree(
        &fixtures.join("_modules"),
        &Path::new(&b.root).join("_modules"),
    );
    b
}

impl Bed {
    async fn verb(&self, line: &str) -> Value {
        let acct = start(&self.dir);
        let mut ctx = super::tests::ctx(&self.dir, Tag::Verb, "", &self.lines);
        ctx.docs = Some(Docs::of(&acct, &code_root()));
        let c = call(&self.dir, "", json!({ "line": line }), Some(ctx));
        let (status, t) = text(routes::dispatch(H::Verb, &c).await).await;
        assert_eq!(status, 200, "`{line}`: {t}");
        serde_json::from_str(&t).unwrap()
    }
}

// A verb lands its writes as one step labelled with its line; undo gives the files back, and the step says what it changed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_verb_is_one_step_that_undo_lays_back() {
    let b = clip_lane("verb-step").await;
    let (project, drums) = (b.file("project.k7x"), b.file("nodes/drums.k7x"));
    let ans = b.verb("project tempo 96").await;
    assert_eq!(ans["refused"], json!(false), "{ans}");
    assert_eq!(ans["log"], json!(["tempo 96 bpm"]));
    assert_eq!(ans["effects"], json!(["reload"]));
    assert!(
        b.file("project.k7x").contains("tempo: 96"),
        "{}",
        b.file("project.k7x")
    );
    let ans = b.verb("event move drums 1:3:1 3:1:1").await;
    assert_eq!(ans["refused"], json!(false), "{ans}");
    assert!(b.file("nodes/drums.k7x").contains("start(\"3:1:1\")"));
    let changed: Vec<&str> = ans["changed"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["path"].as_str())
        .collect();
    assert_eq!(changed, ["nodes/drums.k7x"]);
    let steps = b.steps().await;
    let labels: Vec<&str> = steps
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["label"].as_str())
        .collect();
    assert_eq!(labels.len(), 2, "{steps}");
    assert!(
        labels.contains(&"project tempo 96") && labels.contains(&"event move drums 1:3:1 3:1:1")
    );
    assert_eq!(b.ans(Tag::Undo, json!({})).await.1["applied"], json!(true));
    assert_eq!(b.file("nodes/drums.k7x"), drums);
    assert_eq!(b.ans(Tag::Undo, json!({})).await.1["applied"], json!(true));
    assert_eq!(b.file("project.k7x"), project);
}

// A refusal writes nothing and makes no step; a verb that adds a file adds it on the disk.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_writes_nothing_and_a_new_lane_is_a_new_file() {
    let b = clip_lane("verb-refused").await;
    let drums = b.file("nodes/drums.k7x");
    let ans = b.verb("event move drums 7:1:1 8:1:1").await;
    assert_eq!(ans["refused"], json!(true));
    assert_eq!(ans["landed"], json!(false));
    assert_eq!(b.file("nodes/drums.k7x"), drums);
    assert_eq!(b.steps().await.as_array().map(Vec::len), Some(0));
    let ans = b.verb("track new perc").await;
    assert_eq!(ans["refused"], json!(false), "{ans}");
    assert!(b.has("nodes/perc.k7x"));
    assert!(b.file("graph.k7x").contains("perc"));
    assert_eq!(b.ans(Tag::Undo, json!({})).await.1["applied"], json!(true));
    assert!(!b.has("nodes/perc.k7x"));
}

// A machine verb writes the studio's preferences and is never a step of the project's history.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_machine_verb_writes_the_preferences_outside_the_history() {
    let b = clip_lane("verb-prefs").await;
    let ans = b.verb("system channel set in 3+4 synth").await;
    assert_eq!(ans["refused"], json!(false), "{ans}");
    let prefs = std::fs::read_to_string(b.dir.join("home/preferences.k7x")).unwrap();
    assert!(prefs.contains("synth"), "{prefs}");
    let steps = b.steps().await;
    assert!(
        steps
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["label"] != json!("system channel set in 3+4 synth")),
        "{steps}"
    );
}

/// What the fake front was asked, in order: `METHOD target body`.
type Asked = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

impl Bed {
    /// A line whose routes the fake front answers with `reply(target)`.
    async fn verb_asking(&self, line: &str, reply: fn(&str) -> Value) -> (Value, Vec<String>) {
        let acct = start(&self.dir);
        let mut ctx = super::tests::ctx(&self.dir, Tag::Verb, "", &self.lines);
        ctx.docs = Some(Docs::of(&acct, &code_root()));
        let mut c = call(&self.dir, "", json!({ "line": line }), Some(ctx));
        let asked: Asked = Asked::default();
        let log = asked.clone();
        c.reenter = Some(std::sync::Arc::new(move |m, target, body| {
            log.lock().unwrap().push(format!("{m} {target} {body}"));
            let answer = reply(&target);
            Box::pin(async move { crate::routes::files::json_answer(200, &answer) })
        }));
        let (status, t) = text(routes::dispatch(H::Verb, &c).await).await;
        assert_eq!(status, 200, "`{line}`: {t}");
        let asked = asked.lock().unwrap().clone();
        (serde_json::from_str(&t).unwrap(), asked)
    }
}

// A verb that needs a route's answer runs again with it, and words what it got.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_call_is_asked_of_the_front_and_the_line_runs_again_with_its_answer() {
    let b = clip_lane("verb-call").await;
    let plugins = |_: &str| json!({ "plugins": [{ "name": "Dexed", "format": "VST3", "manufacturer": "Digital Suburban", "role": "instrument" }] });
    let (ans, asked) = b.verb_asking("system plugins list", plugins).await;
    assert_eq!(asked, ["GET /verb-host/plugins null"]);
    assert_eq!(ans["refused"], json!(false), "{ans}");
    assert!(ans.get("pending").is_none(), "{ans}");
    let log = ans["log"].to_string();
    assert!(log.contains("Dexed"), "{log}");
}

// An ask goes after the landing, with the open project as `at`, and the lines it answers are said.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ask_is_sent_after_the_line_and_its_lines_are_said() {
    let b = clip_lane("verb-ask").await;
    let engine = |_: &str| json!({ "lines": ["engine: playing"] });
    let (ans, asked) = b.verb_asking("transport play", engine).await;
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].starts_with("POST /verb-host/engine "), "{asked:?}");
    let body: Value = serde_json::from_str(asked[0].splitn(3, ' ').nth(2).unwrap()).unwrap();
    assert_eq!(body["op"], json!("transport"));
    assert_eq!(body["action"], json!("play"));
    assert!(body["at"].is_string(), "{body}");
    assert!(ans["log"].to_string().contains("engine: playing"), "{ans}");
}

// Completion reads the open project and writes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_line_is_completed_under_the_caret() {
    let b = clip_lane("verb-complete").await;
    let acct = start(&b.dir);
    let mut ctx = super::tests::ctx(&b.dir, Tag::VerbComplete, "", &b.lines);
    ctx.docs = Some(Docs::of(&acct, &code_root()));
    let c = call(&b.dir, "", json!({ "line": "project tem" }), Some(ctx));
    let (status, t) = text(routes::dispatch(H::VerbComplete, &c).await).await;
    assert_eq!(status, 200, "{t}");
    let ans: Value = serde_json::from_str(&t).unwrap();
    assert!(ans["items"].to_string().contains("project tempo"), "{ans}");
}
