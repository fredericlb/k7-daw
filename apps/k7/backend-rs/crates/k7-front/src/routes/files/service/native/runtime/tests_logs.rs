//! The Logs panel's route and the ring's feeds (2620 to 2624): the numbers of the TypeScript, a machine that stalls or
//! never answers, and a line the front says landing in the ring alone.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::routes::files::service::native::callbacks::BoxFuture;
use serde_json::{Value, json};

use super::Runtime;
use super::logs::{ASKS, Heard, Machine, param};
use crate::routes::files::service::native::tests::scratch;
use crate::routes::files::service::native::tests_account::call;

/// A machine that answers `reply` after `delay`, remembering each request and how many asks overlapped.
pub(super) struct Fake {
    reply: Result<Option<Value>, String>,
    delay: Duration,
    seen: Mutex<Vec<Value>>,
    now: Arc<AtomicUsize>,
    most: AtomicUsize,
    /// When set, the ask answers once this is notified, not after `delay`.
    pub(super) gate: Option<Arc<tokio::sync::Notify>>,
}

impl Fake {
    pub(super) fn new(reply: Result<Option<Value>, String>, delay: Duration) -> Arc<Fake> {
        Arc::new(Fake {
            reply,
            delay,
            seen: Mutex::default(),
            now: Arc::new(AtomicUsize::new(0)),
            most: AtomicUsize::new(0),
            gate: None,
        })
    }

    pub(super) fn gated(
        reply: Result<Option<Value>, String>,
    ) -> (Arc<Fake>, Arc<tokio::sync::Notify>) {
        let gate = Arc::new(tokio::sync::Notify::new());
        let f = Fake {
            reply,
            delay: Duration::ZERO,
            seen: Mutex::default(),
            now: Arc::new(AtomicUsize::new(0)),
            most: AtomicUsize::new(0),
            gate: Some(gate.clone()),
        };
        (Arc::new(f), gate)
    }
}

impl Machine for Fake {
    fn ask(
        &self,
        request: Value,
        _last: String,
        heard: Heard,
    ) -> BoxFuture<Result<Option<Value>, String>> {
        self.seen.lock().unwrap().push(request);
        let n = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(n, Ordering::SeqCst);
        let (reply, delay) = (self.reply.clone(), self.delay);
        let now = self.now.clone();
        let gate = self.gate.clone();
        Box::pin(async move {
            match gate {
                Some(g) => g.notified().await,
                None => tokio::time::sleep(delay).await,
            }
            now.fetch_sub(1, Ordering::SeqCst);
            heard();
            reply
        })
    }
}

fn machine_says() -> Value {
    // The wire shape: the supervisor's `log` reply carries the chunk under `raw`.
    json!({ "type": "log", "raw": { "offset": 7, "lines": ["engine up"] } })
}

#[tokio::test]
async fn the_answer_has_the_typescripts_keys_and_the_rings_lines() {
    let m = Fake::new(Ok(Some(machine_says())), Duration::ZERO);
    let rt = Runtime::new(m.clone());
    rt.say("warn", "a conflict");
    let a = rt.logs("tail=5&offset=3").await;
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["offset", "lines", "service"]);
    assert_eq!(
        (a["offset"].clone(), a["lines"].clone()),
        (json!(7), json!(["engine up"]))
    );
    assert_eq!(a["service"]["seq"], 1);
    assert_eq!(a["service"]["lines"][0]["text"], "a conflict");
    assert_eq!(a["service"]["lines"][0]["level"], "warn");
    assert_eq!(
        m.seen.lock().unwrap()[0],
        json!({ "type": "log", "offset": 3.0, "tail": 5.0 })
    );
}

#[tokio::test]
async fn a_repeated_key_is_absent_for_after_and_the_tail_falls_back_to_300() {
    let rt = Runtime::new(Fake::new(Ok(None), Duration::ZERO));
    for i in 0..400 {
        rt.say("info", &format!("line {i}"));
    }
    // `?after=3&after=4` is an array to Express, `Number` of which is NaN: no cursor, the tail's last 300.
    let a = rt.logs("after=3&after=4&tail=1&tail=2").await;
    assert_eq!(a["service"]["lines"].as_array().unwrap().len(), 300);
    assert_eq!(param("after=3&after=4", "after").as_deref(), Some("3,4"));
    assert_eq!(param("tail=1", "after"), None);
    // A key present and empty is `Number("")`, which is 0.
    let a = rt.logs("after=&tail=2000").await;
    assert_eq!(a["service"]["lines"].as_array().unwrap().len(), 400);
    // Nothing bound, and a number nobody can read, are the empty machine half.
    assert_eq!(
        (a["offset"].clone(), a["lines"].clone()),
        (json!(0), json!([]))
    );
}

#[tokio::test]
async fn a_machine_that_answers_nonsense_or_fails_is_the_empty_half() {
    for reply in [
        Ok(Some(json!("text"))),
        Ok(Some(json!({ "raw": { "offset": "x", "lines": [] } }))),
        Ok(Some(json!({ "raw": { "offset": 1, "lines": [1] } }))),
        Ok(Some(json!({ "offset": 7, "lines": ["engine up"] }))),
        Err("jobs agent refused".to_string()),
    ] {
        let rt = Runtime::new(Fake::new(reply, Duration::ZERO));
        let a = rt.logs("").await;
        assert_eq!(
            (a["offset"].clone(), a["lines"].clone()),
            (json!(0), json!([]))
        );
    }
}

#[tokio::test]
async fn a_machine_that_never_answers_is_waited_for_its_bound_and_a_third_ask_none() {
    let m = Fake::new(Ok(Some(machine_says())), Duration::from_secs(3600));
    let rt = Runtime::new_waiting(m.clone(), Duration::from_millis(300));
    let slow: Vec<_> = (0..ASKS + 1)
        .map(|_| {
            let rt = rt.clone();
            tokio::spawn(async move { rt.logs("").await })
        })
        .collect();
    for h in slow {
        // A bound that did not hold would wait the fake's hour: fail here instead (F5).
        let a = tokio::time::timeout(Duration::from_secs(10), h)
            .await
            .expect("the ask's bound did not hold")
            .unwrap();
        assert_eq!(a["offset"], 0);
    }
    assert_eq!(m.most.load(Ordering::SeqCst), ASKS);
    assert_eq!(m.seen.lock().unwrap().len(), ASKS);
    // The slots are given back: the next poll asks again.
    let _ = rt.logs("").await;
    assert_eq!(m.seen.lock().unwrap().len(), ASKS + 1);
}

#[tokio::test]
async fn a_line_the_front_says_is_written_in_the_ring() {
    let dir = scratch("runtime-say");
    let rt = Runtime::new(Fake::new(Ok(None), Duration::ZERO));
    let mut c = call(&dir, "", json!({}), None);
    c.runtime = Some(rt.clone());
    c.say("error", "[songs] push failed");
    let a = rt.logs("").await;
    assert_eq!(a["service"]["lines"][0]["text"], "[songs] push failed");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_job_dropped_with_its_runtime_ends_failed_in_the_ring_and_the_list() {
    let rt = Runtime::new(Fake::new(Ok(None), Duration::ZERO));
    assert!(rt.bg_start("ffmpeg /m/a b.mp4"));
    assert!(!rt.bg_start("ffmpeg /m/a b.mp4"));
    assert!(rt.holds());
    rt.dropped();
    assert!(!rt.holds());
    let a = rt.logs("").await;
    let text = a["service"]["lines"][0]["text"].as_str().unwrap();
    assert!(text.contains("the file service restarted"), "{text}");
    assert_eq!(rt.processes()[0]["state"], "failed");
}

#[tokio::test]
async fn a_hundred_polls_at_once_never_hold_more_than_two_asks() {
    let m = Fake::new(Ok(Some(machine_says())), Duration::from_millis(50));
    let rt = Runtime::new(m.clone());
    let polls: Vec<_> = (0..100)
        .map(|_| {
            let rt = rt.clone();
            tokio::spawn(async move { rt.logs("tail=10").await })
        })
        .collect();
    for p in polls {
        assert!(p.await.unwrap()["service"]["lines"].is_array());
    }
    assert!(m.most.load(Ordering::SeqCst) <= ASKS);
    assert!(m.seen.lock().unwrap().len() < 100);
}

#[tokio::test]
async fn polls_dropped_mid_ask_give_their_slots_back() {
    let (m, gate) = Fake::gated(Ok(Some(machine_says())));
    let rt = Runtime::new_waiting(m.clone(), Duration::from_secs(60));
    for _ in 0..ASKS {
        let rt = rt.clone();
        let h = tokio::spawn(async move { rt.logs("").await });
        while m.now.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        h.abort();
        let _ = h.await;
        m.now.store(0, Ordering::SeqCst);
    }
    assert_eq!(rt.asks.load(Ordering::SeqCst), 0);
    // The next poll reaches the machine, which answers at once.
    gate.notify_one();
    let a = tokio::time::timeout(Duration::from_secs(5), rt.logs(""))
        .await
        .expect("the poll never reached the machine");
    assert_eq!(a["offset"], 7);
}

/// F6: the route answers `resolveWorkspace`'s 400s and 404 before the logs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_logs_route_answers_the_workspace_refusals_then_the_logs() {
    use super::logs::route;
    use super::tests_stop::{Rig, rig};
    use crate::routes::files::service::Who;

    let Rig {
        dir,
        svc,
        instances,
        ..
    } = rig("logs-route");
    let held = svc.held("a@x.org", &instances).await.unwrap();
    let who = Who {
        held: &held,
        user: "a@x.org",
        admin: false,
        device: false,
        log: &svc.log,
        code_root: None,
    };
    let rt = Runtime::new(Fake::new(Ok(None), Duration::ZERO));
    let with = |name: Option<&str>| {
        let mut h = hyper::HeaderMap::new();
        if let Some(n) = name {
            h.insert("x-studio-workspace", n.parse().unwrap());
        }
        h
    };
    std::fs::create_dir_all(held.account.home.join("workspace/real")).unwrap();
    let status = |r: axum::response::Response| r.status().as_u16();
    assert_eq!(status(route(&rt, &who, &with(None), "").await), 400);
    assert_eq!(status(route(&rt, &who, &with(Some("a/b")), "").await), 400);
    assert_eq!(status(route(&rt, &who, &with(Some("gone")), "").await), 404);
    assert_eq!(status(route(&rt, &who, &with(Some("real")), "").await), 200);
    let _ = std::fs::remove_dir_all(dir);
}

/// 2631: a job whose `jobs.ask` is in flight keeps its lease past ten minutes of silence.
#[tokio::test]
async fn an_ask_in_flight_holds_the_lease() {
    let (m, _gate) = Fake::gated(Ok(None));
    let rt = Runtime::new_waiting(m, Duration::from_millis(100));
    assert!(rt.bg_start("render pic"));
    let ask = flight(&rt);
    while rt.flights.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    rt.skew.store(11 * 60, Ordering::SeqCst);
    assert!(rt.holds(), "the lease lapsed under an ask in flight");
    rt.lapsed();
    let said = tokio::time::timeout(Duration::from_secs(5), rt.logs("")).await;
    let said = said.expect("logs hung");
    assert!(said["service"]["lines"].as_array().unwrap().is_empty());
    ask.abort();
}

/// 2631: each reply the machine's agent sees renews the lease, and silence after the ask lapses it.
#[tokio::test]
async fn each_reply_renews_the_lease_and_silence_after_the_ask_lapses_it() {
    let (m, gate) = Fake::gated(Ok(None));
    let rt = Runtime::new(m);
    assert!(rt.bg_start("render pic"));
    let ask = flight(&rt);
    while rt.flights.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // The reply is heard at 9 minutes; 6 minutes later the lease still holds.
    rt.skew.store(9 * 60, Ordering::SeqCst);
    gate.notify_one();
    ask.await.unwrap();
    rt.skew.store(15 * 60, Ordering::SeqCst);
    assert!(rt.holds(), "a reply did not renew the lease");
    rt.skew.store(30 * 60, Ordering::SeqCst);
    assert!(!rt.holds());
}

fn flight(rt: &std::sync::Arc<Runtime>) -> tokio::task::JoinHandle<Option<Result<Value, String>>> {
    let rt = rt.clone();
    let request = json!({ "request": { "type": "render" }, "last": "rendered" });
    tokio::spawn(async move { rt.ask("jobs.ask", &request).await })
}
