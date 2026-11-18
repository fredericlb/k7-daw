//! The gate held to `relayGate.test.ts`'s "the gate", a manual clock standing for vitest's fake timers.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;

#[derive(Default)]
struct Seen {
    holds: bool,
    may_read: bool,
    asked: usize,
    sent: Vec<String>,
    closed: Option<(u16, String)>,
}

struct Probe(Arc<Mutex<Seen>>);

impl Sinks for Probe {
    fn holds(&self) -> bool {
        let mut s = self.0.lock().unwrap();
        s.asked += 1;
        s.holds
    }
    fn may_read_token(&self) -> bool {
        self.0.lock().unwrap().may_read
    }
    fn to_engine(&mut self, f: Frame) {
        self.0
            .lock()
            .unwrap()
            .sent
            .push(String::from_utf8_lossy(&f.data).into_owned());
    }
    fn close(&mut self, code: u16, reason: &str) {
        self.0.lock().unwrap().closed = Some((code, reason.into()));
    }
}

fn text(v: Value) -> Frame {
    Frame::text(v.to_string())
}

fn gate(grace: i64) -> (Gate<Probe>, Arc<Mutex<Seen>>, Clock) {
    let seen = Arc::new(Mutex::new(Seen {
        may_read: true,
        ..Default::default()
    }));
    let clock = Clock::default();
    clock.set_now(1_000_000);
    (
        Gate::new(Probe(seen.clone()), grace, clock.clone()),
        seen,
        clock,
    )
}

/// Moves the clock by `ms`, ticking as an interval of POLL_MS would.
fn advance(g: &mut Gate<Probe>, clock: &Clock, ms: i64) {
    let mut done = 0;
    while done + POLL_MS as i64 <= ms {
        done += POLL_MS as i64;
        clock.set_now(clock.now_ms() + POLL_MS as i64);
        if g.waiting() {
            g.tick();
        }
    }
    clock.set_now(clock.now_ms() + ms - done);
}

#[test]
fn passes_the_probe_and_its_answers_before_any_claim_and_nothing_else() {
    let (mut g, seen, _) = gate(1000);
    for t in ["pair", "whoami", "list-plugins", "plugin-scan"] {
        g.from_client(text(json!({ "type": t })));
    }
    assert_eq!(seen.lock().unwrap().sent.len(), 4);
    assert!(
        g.from_engine(text(json!({ "type": "plugin-scan", "scanning": true })))
            .is_some()
    );
    assert!(
        g.from_engine(text(json!({ "type": "whoami", "id": "e1" })))
            .is_some()
    );
    assert!(
        g.from_engine(text(json!({ "type": "plugins", "plugins": [] })))
            .is_some()
    );
    assert!(
        g.from_engine(text(json!({ "type": "state", "playing": true })))
            .is_none()
    );
    assert!(g.from_engine(Frame::binary(vec![1u8, 2])).is_none());
}

#[test]
fn strips_the_token_from_a_whoami_for_an_account_that_may_not_use_the_engine() {
    let (mut g, seen, _) = gate(1000);
    let who = text(json!({ "type": "whoami", "id": "e1", "token": "tok", "bufferSize": 64 }));
    let read = |f: Option<Frame>| serde_json::from_slice::<Value>(&f.unwrap().data).unwrap();
    assert_eq!(read(g.from_engine(who.clone()))["token"], "tok");
    seen.lock().unwrap().may_read = false;
    assert_eq!(
        read(g.from_engine(who.clone())),
        json!({ "type": "whoami", "id": "e1", "bufferSize": 64 })
    );
    seen.lock().unwrap().holds = true;
    assert!(read(g.from_engine(who)).get("token").is_none());
}

#[test]
fn holds_what_comes_next_in_order_and_lets_it_through_once_the_claim_lands() {
    let (mut g, seen, clock) = gate(1000);
    g.from_client(text(json!({ "type": "hello", "role": "driver" })));
    g.from_client(text(json!({ "type": "whoami" })));
    assert!(seen.lock().unwrap().sent.is_empty());
    seen.lock().unwrap().holds = true;
    advance(&mut g, &clock, 250);
    let types: Vec<String> = seen
        .lock()
        .unwrap()
        .sent
        .iter()
        .map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(types, ["hello", "whoami"]);
    assert!(g.from_engine(text(json!({ "type": "state" }))).is_some());
    assert!(seen.lock().unwrap().closed.is_none());
}

#[test]
fn closes_the_socket_when_no_claim_comes() {
    let (mut g, seen, clock) = gate(1000);
    g.from_client(text(json!({ "type": "transport", "action": "play" })));
    advance(&mut g, &clock, 1200);
    let s = seen.lock().unwrap();
    assert!(s.sent.is_empty());
    assert_eq!(s.closed.as_ref().map(|c| c.0), Some(NO_CLAIM_CLOSE));
}

#[test]
fn stops_a_holder_that_lost_its_claim_at_its_next_frame() {
    let (mut g, seen, _) = gate(1000);
    seen.lock().unwrap().holds = true;
    g.from_client(text(json!({ "type": "transport", "action": "play" })));
    seen.lock().unwrap().holds = false;
    g.from_client(text(json!({ "type": "transport", "action": "stop" })));
    assert_eq!(seen.lock().unwrap().sent.len(), 1);
}

#[test]
fn reuses_a_claim_for_the_engines_frames_briefly_then_asks_again() {
    let (mut g, seen, clock) = gate(CLAIM_GRACE_MS);
    seen.lock().unwrap().holds = true;
    let audio = Frame::binary(vec![1u8, 2, 3, 4]);
    for _ in 0..50 {
        assert!(g.from_engine(audio.clone()).is_some());
    }
    assert_eq!(seen.lock().unwrap().asked, 1);
    seen.lock().unwrap().holds = false;
    advance(&mut g, &clock, 300);
    assert!(g.from_engine(audio).is_none());
}

#[test]
fn too_much_held_closes_at_once() {
    let (mut g, seen, _) = gate(1000);
    for n in 0..=MAX_HELD_FRAMES {
        g.from_client(text(json!({ "type": "transport", "n": n })));
    }
    assert_eq!(
        seen.lock().unwrap().closed.as_ref().map(|c| c.0),
        Some(NO_CLAIM_CLOSE)
    );
}

#[test]
fn a_whoami_it_cannot_read_never_carries_the_token_to_an_account_that_may_not_use_the_engine() {
    let (mut g, seen, _) = gate(1000);
    seen.lock().unwrap().holds = true;
    seen.lock().unwrap().may_read = false;
    let lone = Frame::text(r#"{"type":"whoami","token":"tok","name":"\ud800"}"#);
    assert!(g.from_engine(lone.clone()).is_none());
    seen.lock().unwrap().may_read = true;
    assert!(g.from_engine(lone).is_some());
}
