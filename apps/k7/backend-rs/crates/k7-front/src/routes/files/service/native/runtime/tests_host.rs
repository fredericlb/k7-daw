//! The calls the stand-ins make (2630 to 2633, 2660): the registry's start and end, the lines they say, and the
//! machine ask's three answers.

use std::time::Duration;

use serde_json::{Value, json};

use super::Runtime;
use super::tests_logs::Fake;

fn runtime(reply: Result<Option<Value>, String>) -> std::sync::Arc<Runtime> {
    Runtime::new(Fake::new(reply, Duration::ZERO))
}

async fn start(rt: &std::sync::Arc<Runtime>) -> Option<Result<Value, String>> {
    rt.ask("bg.start", &json!({ "tag": "ffmpeg /a b/c.mp4" }))
        .await
}

#[tokio::test]
async fn a_tag_starts_once_is_listed_at_once_and_ends_in_a_line() {
    let rt = runtime(Ok(None));
    assert_eq!(start(&rt).await, Some(Ok(json!({ "value": true }))));
    assert_eq!(start(&rt).await, Some(Ok(json!({ "value": false }))));
    let list = rt.processes();
    assert_eq!(
        (list.len(), list[0]["state"].clone()),
        (1, json!("running"))
    );
    assert_eq!(list[0]["label"], "ffmpeg c.mp4");
    assert!(rt.tell(
        "bg.end",
        &json!({ "tag": "ffmpeg /a b/c.mp4", "line": "c.mp4 done" })
    ));
    // A second end says nothing: the tag no longer runs.
    assert!(rt.tell(
        "bg.end",
        &json!({ "tag": "ffmpeg /a b/c.mp4", "error": "late" })
    ));
    let said = rt.logs("").await;
    let lines = said["service"]["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["text"], "   ✓ c.mp4 done");
    assert_eq!(rt.processes()[0]["state"], "done");
    // Ended, the tag may start again.
    assert_eq!(start(&rt).await, Some(Ok(json!({ "value": true }))));
}

#[tokio::test]
async fn a_failed_job_says_its_error_and_ten_thousand_starts_of_one_tag_are_one_job() {
    let rt = runtime(Ok(None));
    let mut started = 0;
    for _ in 0..10_000 {
        if rt.bg_start("transcode x") {
            started += 1;
        }
    }
    assert_eq!((started, rt.processes().len()), (1, 1));
    assert_eq!(
        rt.counters
            .background
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    rt.tell(
        "bg.end",
        &json!({ "tag": "transcode x", "error": "ffmpeg exited 1" }),
    );
    let said = rt.logs("").await;
    assert_eq!(
        said["service"]["lines"][0]["text"],
        "   ✗ transcode x failed: ffmpeg exited 1"
    );
    assert_eq!(said["service"]["lines"][0]["level"], "error");
}

#[tokio::test]
async fn a_progress_after_the_end_is_dropped() {
    let rt = runtime(Ok(None));
    rt.bg_start("t");
    rt.tell(
        "bg.progress",
        &json!({ "tag": "t", "fraction": 0.5, "detail": "half" }),
    );
    assert_eq!(rt.processes()[0]["progress"], 0.5);
    rt.tell("bg.end", &json!({ "tag": "t" }));
    rt.tell("bg.progress", &json!({ "tag": "t", "fraction": 0.9 }));
    assert!(
        rt.processes()[0]
            .get("progress")
            .is_none_or(|p| *p != json!(0.9))
    );
}

#[tokio::test]
async fn the_machine_ask_answers_none_bound_a_reply_or_a_job_error() {
    let ask = json!({ "request": { "type": "log" }, "last": "log" });
    let none = runtime(Ok(None)).ask("jobs.ask", &ask).await;
    assert_eq!(none, Some(Ok(json!({ "value": null }))));
    let reply = runtime(Ok(Some(json!({ "type": "log", "offset": 1 }))))
        .ask("jobs.ask", &ask)
        .await;
    assert_eq!(
        reply,
        Some(Ok(json!({ "value": { "type": "log", "offset": 1 } })))
    );
    let failed = runtime(Err("the machine said no".into()))
        .ask("jobs.ask", &ask)
        .await;
    assert_eq!(
        failed,
        Some(Ok(json!({ "jobError": "the machine said no" })))
    );
}

#[tokio::test]
async fn a_kind_that_is_not_ours_falls_through() {
    let rt = runtime(Ok(None));
    assert!(rt.ask("engineBinary", &json!({})).await.is_none());
    assert!(!rt.tell("engine.start", &json!({})));
    assert!(
        rt.logs("").await["service"]["lines"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_loops_console_line_reaches_the_ring_with_its_level() {
    let rt = runtime(Ok(None));
    for (level, line) in [("warn", "w"), ("error", "e"), ("info", "i"), ("odd", "o")] {
        assert!(rt.tell("log", &json!({ "line": line, "level": level })));
    }
    let said = rt.logs("").await;
    let got: Vec<(String, String)> = said["service"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["level"].as_str().unwrap().into(),
                l["text"].as_str().unwrap().into(),
            )
        })
        .collect();
    let want = [("warn", "w"), ("error", "e"), ("info", "i"), ("info", "o")]
        .map(|(a, b)| (a.to_string(), b.to_string()));
    assert_eq!(got, want);
}

#[tokio::test]
async fn the_module_watch_is_joined_asked_moved_and_released_by_the_stand_in_calls() {
    let tmp = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-host-watch-{}", std::process::id()));
    let (p, q) = (tmp.join("p"), tmp.join("pq"));
    for d in [&p, &q] {
        std::fs::create_dir_all(d.join("nodes")).unwrap();
    }
    let rt = runtime(Ok(None));
    let join = |d: &std::path::Path| json!({ "dir": d.display().to_string(), "roots": [d.join("nodes").display().to_string()], "feed": null });
    let still = |d: &std::path::Path| json!({ "dir": d.display().to_string() });
    for d in [&p, &q] {
        assert_eq!(
            rt.ask("watch.join", &join(d)).await,
            Some(Ok(json!({ "value": true })))
        );
    }
    // The first ask walks, the second is still, a write of ours moves every join (2640).
    let yes = |v: bool| Some(Ok(json!({ "value": v })));
    assert_eq!(rt.ask("watch.still", &still(&p)).await, yes(false));
    assert_eq!(rt.ask("watch.still", &still(&p)).await, yes(true));
    assert!(rt.tell("watch.moved", &json!({})));
    assert_eq!(rt.ask("watch.still", &still(&p)).await, yes(false));
    // A release under `p` is by path component: `pq` beside it stays (2641).
    let gone = rt
        .ask(
            "watch.release",
            &json!({ "under": p.display().to_string() }),
        )
        .await;
    assert_eq!(gone, Some(Ok(json!({ "value": 1 }))));
    assert_eq!(rt.watch.joined(), 1);
    assert_eq!(rt.ask("watch.still", &still(&p)).await, yes(false));
    // The runtime dropped lets go of the rest.
    rt.dropped();
    assert_eq!(rt.watch.joined(), 0);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn a_logs_answer_counts_in_the_report() {
    let a = super::report::totals()
        .logs
        .load(std::sync::atomic::Ordering::Relaxed);
    super::report::logs_answered();
    assert!(
        super::report::totals()
            .logs
            .load(std::sync::atomic::Ordering::Relaxed)
            > a
    );
    assert!(super::report::line().starts_with("[files] native runtime: background="));
}
