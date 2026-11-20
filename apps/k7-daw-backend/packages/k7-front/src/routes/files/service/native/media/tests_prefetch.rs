//! tests_prefetch.rs — the prefetch's timer over a jobs agent of its own: what it asks and when, and that a stop or the
//! account's reap drops the job in flight. What a reader gets wrong: every wait is a poll of a count, never a fixed sleep.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use super::Counts;
use super::prefetch::{Prefetch, Source};

const TICK: Duration = Duration::from_millis(20);

/// A jobs path that records each request and answers `prefetched`, an error, or nothing at all.
struct Agent {
    url: String,
    asked: Arc<Mutex<Vec<Value>>>,
    closed: Arc<AtomicU64>,
    mode: Arc<Mutex<&'static str>>,
    task: JoinHandle<()>,
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Agent {
    async fn start() -> Agent {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let (asked, closed): (Arc<Mutex<Vec<Value>>>, Arc<AtomicU64>) = Default::default();
        let mode = Arc::new(Mutex::new("ok"));
        let (a, c, m) = (asked.clone(), closed.clone(), mode.clone());
        let task = tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let (a, c, m) = (a.clone(), c.clone(), m.clone());
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(tcp).await else {
                        return;
                    };
                    let (mut write, mut read) = ws.split();
                    while let Some(Ok(frame)) = read.next().await {
                        let Message::Text(t) = frame else { continue };
                        let req: Value = serde_json::from_str(t.as_str()).unwrap();
                        a.lock().unwrap().push(req.clone());
                        let reply = match *m.lock().unwrap() {
                            "ok" => json!({ "type": "prefetched", "id": req["id"] }),
                            "error" => {
                                json!({ "type": "error", "id": req["id"], "error": "disk full" })
                            }
                            _ => continue,
                        };
                        let _ = write.send(Message::text(reply.to_string())).await;
                    }
                    c.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        Agent {
            url,
            asked,
            closed,
            mode,
            task,
        }
    }

    fn asked(&self) -> Vec<Value> {
        self.asked.lock().unwrap().clone()
    }
}

struct Fixed {
    url: Mutex<Option<String>>,
    files: Mutex<Vec<String>>,
    said: Mutex<Vec<String>>,
}

impl Fixed {
    fn new(url: Option<&str>, files: &[&str]) -> Arc<Fixed> {
        Arc::new(Fixed {
            url: Mutex::new(url.map(str::to_string)),
            files: Mutex::new(files.iter().map(|f| f.to_string()).collect()),
            said: Mutex::default(),
        })
    }
}

impl Source for Fixed {
    fn jobs_url(&self) -> Option<String> {
        self.url.lock().unwrap().clone()
    }

    fn named_files(&self) -> Vec<String> {
        self.files.lock().unwrap().clone()
    }

    fn say(&self, line: &str) {
        self.said.lock().unwrap().push(line.to_string());
    }
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never: {what}");
}

fn nop() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(|| {})
}

fn prefetches(c: &Counts) -> u64 {
    c.prefetches.load(Ordering::Relaxed)
}

#[tokio::test]
async fn the_named_files_are_asked_once_and_again_only_when_the_list_or_the_machine_moves() {
    let agent = Agent::start().await;
    let src = Fixed::new(Some(&agent.url), &["/m/a/1.wav", "/m/a/2.wav"]);
    let (counts, pre) = (Arc::new(Counts::default()), Prefetch::default());
    pre.start(src.clone(), counts.clone(), nop(), Duration::ZERO, TICK);
    until("the first ask", || agent.asked().len() == 1).await;
    assert_eq!(agent.asked()[0]["type"], "prefetch");
    assert_eq!(
        agent.asked()[0]["files"],
        json!(["/m/a/1.wav", "/m/a/2.wav"])
    );
    tokio::time::sleep(TICK * 8).await;
    assert_eq!(
        agent.asked().len(),
        1,
        "a list that did not move is not asked again"
    );
    src.files.lock().unwrap().push("/m/b/3.wav".into());
    until("the moved list", || agent.asked().len() == 2).await;
    let other = Agent::start().await;
    *src.url.lock().unwrap() = Some(other.url.clone());
    until("the moved machine", || other.asked().len() == 1).await;
    assert_eq!(prefetches(&counts), 3);
    assert!(counts.line().contains("prefetches=3"));
}

#[tokio::test]
async fn nothing_is_asked_without_a_machine_or_without_a_song() {
    let agent = Agent::start().await;
    let (counts, pre) = (Arc::new(Counts::default()), Prefetch::default());
    let none = Fixed::new(None, &["/m/a.wav"]);
    pre.start(none, counts.clone(), nop(), Duration::ZERO, TICK);
    tokio::time::sleep(TICK * 6).await;
    pre.stop();
    let empty = Fixed::new(Some(&agent.url), &[]);
    pre.start(empty, counts.clone(), nop(), Duration::ZERO, TICK);
    tokio::time::sleep(TICK * 6).await;
    assert!(agent.asked().is_empty());
    assert_eq!(prefetches(&counts), 0);
}

#[tokio::test]
async fn a_refused_job_is_said_once_and_asked_again() {
    let agent = Agent::start().await;
    *agent.mode.lock().unwrap() = "error";
    let src = Fixed::new(Some(&agent.url), &["/m/a.wav"]);
    let (counts, pre) = (Arc::new(Counts::default()), Prefetch::default());
    pre.start(src.clone(), counts.clone(), nop(), Duration::ZERO, TICK);
    until("three asks", || agent.asked().len() >= 3).await;
    let said = src.said.lock().unwrap().clone();
    assert_eq!(said.iter().filter(|l| l.ends_with("disk full")).count(), 1);
    assert_eq!(said[0], "   record prefetch: machine=true files=1");
    *agent.mode.lock().unwrap() = "ok";
    let n = agent.asked().len();
    until("the settled ask", || agent.asked().len() > n).await;
    tokio::time::sleep(TICK * 8).await;
    assert!(agent.asked().len() <= n + 2, "settled once it worked");
}

#[tokio::test]
async fn a_stop_drops_the_job_in_flight() {
    let agent = Agent::start().await;
    *agent.mode.lock().unwrap() = "never";
    let src = Fixed::new(Some(&agent.url), &["/m/a.wav"]);
    let (counts, pre) = (Arc::new(Counts::default()), Prefetch::default());
    pre.start(src, counts, nop(), Duration::ZERO, TICK);
    until("the job out", || pre.asking()).await;
    // The agent must hold the socket before the stop, or there is no close to count.
    until("the ask received", || agent.asked().len() == 1).await;
    pre.stop();
    until("the socket closed", || {
        agent.closed.load(Ordering::SeqCst) == 1
    })
    .await;
    assert!(!pre.asking() && !pre.started());
    tokio::time::sleep(TICK * 6).await;
    assert_eq!(agent.asked().len(), 1, "no timer outlives the stop");
}

#[tokio::test]
async fn the_accounts_reap_drops_the_job_and_the_timer() {
    let agent = Agent::start().await;
    *agent.mode.lock().unwrap() = "never";
    let src = Fixed::new(Some(&agent.url), &["/m/a.wav"]);
    let media = super::Media::new(
        super::Config::of(&Default::default(), std::path::Path::new("/nonexistent")),
        Arc::new(|| {}),
    );
    media
        .prefetch
        .start(src, media.counts.clone(), nop(), Duration::ZERO, TICK);
    until("the job out", || media.prefetch.asking()).await;
    until("the ask received", || agent.asked().len() == 1).await;
    drop(media);
    until("the socket closed", || {
        agent.closed.load(Ordering::SeqCst) == 1
    })
    .await;
    tokio::time::sleep(TICK * 6).await;
    assert_eq!(agent.asked().len(), 1);
}
