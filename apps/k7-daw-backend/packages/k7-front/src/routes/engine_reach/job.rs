//! job.rs — asking an engine's machine to do something over a jobs channel, one request with an id of its own, its replies as they come, the
//! last one settling it. What a reader gets wrong: a reply naming another id, or no object at all, is not a reply.

use std::time::Duration;

use k7_mediatek::ecma::js_string;
use serde_json::{Map, Value};
use tokio::time::Instant;

use super::engines::{Link, LinkEvent};

/// A job is given up after this long in all (`JOB_TIMEOUT_MS`).
pub const JOB_TOTAL: Duration = Duration::from_secs(60 * 60);
/// …or after this long without a reply (`JOB_IDLE_MS`).
pub const JOB_IDLE: Duration = Duration::from_secs(15 * 60);

/// `JobError`: the machine refused, went away, or ran out of time; its message is the caller's to show.
#[derive(Debug, PartialEq)]
pub struct JobError(pub String);

/// A duration in minutes as JavaScript prints `ms / 60_000`.
fn minutes(d: Duration) -> String {
    let m = d.as_millis() as f64 / 60_000.0;
    k7_js::number::to_string(m)
}

fn random_id() -> String {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("the system's random source");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// What a job needs of its channel: text out, frames in until it closes, and an end. The tunnel's `Link` and a socket to a
/// bound loopback path both are one (2212).
pub trait JobChannel: Send {
    fn send_text(&mut self, s: String);
    /// The next frame's bytes; `None` once the channel is closed.
    fn recv(&mut self) -> impl std::future::Future<Output = Option<Vec<u8>>> + Send;
    fn close(&mut self);
}

impl JobChannel for Link {
    fn send_text(&mut self, s: String) {
        self.tx.text(s);
    }

    async fn recv(&mut self) -> Option<Vec<u8>> {
        match self.rx.recv().await {
            Some(LinkEvent::Frame(f)) => Some(f.data.to_vec()),
            Some(LinkEvent::Closed) | None => None,
        }
    }

    fn close(&mut self) {
        self.tx.close();
    }
}

/// Send `body` on `link` and resolve with the reply `is_last` accepts; every reply goes to `on_reply` first.
pub async fn request_job<C: JobChannel>(
    link: &mut C,
    mut body: Map<String, Value>,
    is_last: impl Fn(&Map<String, Value>) -> bool,
    mut on_reply: impl FnMut(&Map<String, Value>),
    total: Duration,
    idle: Duration,
) -> Result<Map<String, Value>, JobError> {
    let id = random_id();
    body.insert("id".into(), Value::String(id.clone()));
    link.send_text(k7_mediatek::json::stringify(&Value::Object(body)));
    let deadline = Instant::now() + total;
    let mut quiet = Instant::now() + idle;
    loop {
        // The total wins a tie: its timer was set first.
        let (at, why) = if deadline <= quiet {
            (
                deadline,
                format!(
                    "the engine's machine did not finish within {} min",
                    minutes(total)
                ),
            )
        } else {
            (
                quiet,
                format!(
                    "the engine's machine said nothing for {} min",
                    minutes(idle)
                ),
            )
        };
        let ev = tokio::select! {
            ev = link.recv() => ev,
            _ = tokio::time::sleep_until(at) => {
                link.close();
                return Err(JobError(why));
            }
        };
        let Some(data) = ev else {
            return Err(JobError(
                "the engine's machine went away before answering".into(),
            ));
        };
        let Ok(Value::Object(reply)) = serde_json::from_slice::<Value>(&data) else {
            continue;
        };
        if reply.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            continue;
        }
        quiet = Instant::now() + idle;
        if reply.get("type").and_then(Value::as_str) == Some("error") {
            let said = match reply.get("error") {
                None | Some(Value::Null) => "the engine's machine refused".to_string(),
                Some(v) => js_string(v),
            };
            return Err(JobError(said));
        }
        on_reply(&reply);
        if is_last(&reply) {
            return Ok(reply);
        }
    }
}

/// Whether a reply's `type` is `kind`.
pub fn of_type(kind: &'static str) -> impl Fn(&Map<String, Value>) -> bool {
    move |r| r.get("type").and_then(Value::as_str) == Some(kind)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::routes::engine_reach::engines::Reach;
    use crate::routes::engine_reach::engines_tests::{agent, core, read, say};
    use k7_engine::tunnel::frames::Kind;
    use serde_json::json;

    /// A channel whose replies a test hands out on a clock it moves.
    struct Fake {
        sent: Arc<Mutex<Vec<String>>>,
        closed: Arc<AtomicBool>,
        replies: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    }

    /// The test's end of a `Fake`: what it was sent, whether it was closed, and the replies to give.
    struct Hand {
        sent: Arc<Mutex<Vec<String>>>,
        closed: Arc<AtomicBool>,
        tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    }

    impl Hand {
        /// A reply carrying the id of the request sent.
        fn say(&self, mut reply: Value) {
            let sent = self.sent.lock().unwrap()[0].clone();
            reply["id"] = serde_json::from_str::<Value>(&sent).unwrap()["id"].clone();
            let _ = self.tx.send(reply.to_string().into_bytes());
        }

        fn closed(&self) -> bool {
            self.closed.load(Ordering::SeqCst)
        }
    }

    impl JobChannel for Fake {
        fn send_text(&mut self, s: String) {
            self.sent.lock().unwrap().push(s);
        }

        async fn recv(&mut self) -> Option<Vec<u8>> {
            self.replies.recv().await
        }

        fn close(&mut self) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }

    fn fake() -> (Fake, Hand) {
        let (tx, replies) = tokio::sync::mpsc::unbounded_channel();
        let sent: Arc<Mutex<Vec<String>>> = Arc::default();
        let closed: Arc<AtomicBool> = Arc::default();
        (
            Fake {
                sent: sent.clone(),
                closed: closed.clone(),
                replies,
            },
            Hand { sent, closed, tx },
        )
    }

    async fn pass(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_a_silence_and_closes_the_channel() {
        let (mut chan, hand) = fake();
        let ask = tokio::spawn(async move {
            request_job(
                &mut chan,
                Map::new(),
                of_type("done"),
                |_| {},
                Duration::from_secs(10),
                Duration::from_millis(1000),
            )
            .await
        });
        pass(800).await;
        hand.say(json!({ "type": "progress" }));
        pass(800).await;
        assert!(!hand.closed());
        pass(300).await;
        assert!(ask.await.unwrap().is_err());
        assert!(hand.closed());
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_its_whole_budget_however_often_it_says_something() {
        let (mut chan, hand) = fake();
        let ask = tokio::spawn(async move {
            request_job(
                &mut chan,
                Map::new(),
                of_type("done"),
                |_| {},
                Duration::from_millis(3000),
                Duration::from_millis(1000),
            )
            .await
        });
        for _ in 0..4 {
            pass(800).await;
            hand.say(json!({ "type": "progress" }));
        }
        let said = ask.await.unwrap().unwrap_err().0;
        assert!(said.contains("did not finish"), "{said}");
        assert!(hand.closed());
    }

    #[tokio::test(start_paused = true)]
    async fn resolves_on_the_last_reply_and_leaves_the_channel_open() {
        let (mut chan, hand) = fake();
        let ask = tokio::spawn(async move {
            let r = request_job(
                &mut chan,
                Map::new(),
                of_type("done"),
                |_| {},
                JOB_TOTAL,
                JOB_IDLE,
            )
            .await;
            (r, chan)
        });
        pass(10).await;
        hand.say(json!({ "type": "done" }));
        let (r, _chan) = ask.await.unwrap();
        assert_eq!(r.unwrap()["type"], "done");
        pass(24 * 60 * 60_000).await;
        assert!(!hand.closed());
    }

    #[test]
    fn prints_minutes_as_javascript() {
        assert_eq!(minutes(Duration::from_secs(600)), "10");
        assert_eq!(minutes(Duration::from_secs(30)), "0.5");
    }

    #[tokio::test]
    async fn settles_on_the_last_reply_of_its_own_id_and_refuses_on_an_error() {
        let core = core();
        let mut agent = agent(&core, "e1").await;
        let reach = Reach(&core);
        let mut link = reach
            .open("e1", vec![], Kind::Jobs, Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        let ask = tokio::spawn(async move {
            let mut seen = 0;
            let r = request_job(
                &mut link,
                json!({ "type": "slots" }).as_object().unwrap().clone(),
                of_type("slots"),
                |_| seen += 1,
                JOB_TOTAL,
                JOB_IDLE,
            )
            .await;
            (r, seen, link)
        });
        let open = read(&mut agent).await;
        let ch = open["ch"].clone();
        assert_eq!(open["kind"], "jobs");
        say(&mut agent, json!({ "t": "opened", "ch": ch })).await;
        let sent = read(&mut agent).await;
        let body: Value = serde_json::from_str(sent["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["type"], "slots");
        let id = body["id"].as_str().unwrap().to_string();
        for text in [
            json!({ "type": "slots", "id": "other" }).to_string(),
            "not json".to_string(),
            json!({ "type": "progress", "id": id }).to_string(),
            json!({ "type": "slots", "id": id, "slots": [] }).to_string(),
        ] {
            say(&mut agent, json!({ "t": "data", "ch": ch, "text": text })).await;
        }
        let (r, seen, mut link) = ask.await.unwrap();
        assert_eq!(r.unwrap()["slots"], json!([]));
        assert_eq!(seen, 2);
        let refused = tokio::spawn(async move {
            request_job(
                &mut link,
                Map::new(),
                of_type("x"),
                |_| {},
                JOB_TOTAL,
                JOB_IDLE,
            )
            .await
        });
        let sent = read(&mut agent).await;
        let body: Value = serde_json::from_str(sent["text"].as_str().unwrap()).unwrap();
        let reply = json!({ "type": "error", "id": body["id"], "error": "no slot" }).to_string();
        say(&mut agent, json!({ "t": "data", "ch": ch, "text": reply })).await;
        assert_eq!(refused.await.unwrap(), Err(JobError("no slot".into())));
    }
}
