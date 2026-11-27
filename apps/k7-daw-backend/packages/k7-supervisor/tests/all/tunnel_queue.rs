//! tunnel_queue.rs — a channel's serial queue and the shedding of a slow peer's frames: a drain runs what was
//! queued, in order, then its last task once, and refuses what comes after; a task that never ends cannot hold it
//! past its bound; only a superseded frame is dropped, and only past the mark.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k7_supervisor::tunnel::link::Link;
use k7_supervisor::tunnel::queue::SerialQueue;
use k7_supervisor::tunnel::shed::{
    DROP_ABOVE_BYTES, FrameShedder, SHED_REPORT_EVERY, droppable_engine_frame,
};
use k7_supervisor::tunnel::wire::HIGH_WATER_BYTES;
use serde_json::json;
use tokio::sync::Notify;

use crate::hub::until;

const MS: fn(u64) -> Duration = Duration::from_millis;

#[tokio::test]
async fn drain_runs_what_is_queued_then_last() {
    let (q, order, lasts) = (
        SerialQueue::new(),
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(AtomicUsize::new(0)),
    );
    let push = |n: i32| {
        let order = order.clone();
        async move { order.lock().unwrap().push(n) }
    };
    let slow = order.clone();
    q.push(async move {
        tokio::time::sleep(MS(30)).await;
        slow.lock().unwrap().push(1);
    });
    q.push(push(2));
    let (o, l) = (order.clone(), lasts.clone());
    q.drain(MS(2000), move || {
        o.lock().unwrap().push(9);
        l.fetch_add(1, Ordering::SeqCst);
    });
    q.push(push(3));
    until(|| lasts.load(Ordering::SeqCst) == 1, 1000).await;
    tokio::time::sleep(MS(50)).await;
    assert_eq!(*order.lock().unwrap(), [1, 2, 9]);
    assert_eq!(lasts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_stuck_task_cannot_hold_the_drain_past_its_bound() {
    let (q, lasts, after) = (
        SerialQueue::new(),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let never = Arc::new(Notify::new());
    let held = never.clone();
    q.push(async move { held.notified().await });
    let a = after.clone();
    q.push(async move {
        a.fetch_add(1, Ordering::SeqCst);
    });
    let (started, l) = (Instant::now(), lasts.clone());
    q.drain(MS(100), move || {
        l.fetch_add(1, Ordering::SeqCst);
    });
    until(|| lasts.load(Ordering::SeqCst) == 1, 1000).await;
    assert!(started.elapsed() < MS(900));
    never.notify_waiters();
    tokio::time::sleep(MS(50)).await;
    assert_eq!(
        (after.load(Ordering::SeqCst), lasts.load(Ordering::SeqCst)),
        (0, 1)
    );
}

#[tokio::test]
async fn a_closed_queue_drains_nothing_and_a_panicking_task_costs_only_itself() {
    let (q, lasts, ran) = (
        SerialQueue::new(),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    q.close();
    let l = lasts.clone();
    q.drain(MS(50), move || {
        l.fetch_add(1, Ordering::SeqCst);
    });
    tokio::time::sleep(MS(150)).await;
    assert_eq!(lasts.load(Ordering::SeqCst), 0);
    let q = SerialQueue::new();
    q.push(async { panic!("a bad frame") });
    let r = ran.clone();
    q.push(async move {
        r.fetch_add(1, Ordering::SeqCst);
    });
    until(|| ran.load(Ordering::SeqCst) == 1, 1000).await;
}

fn state() -> String {
    json!({"type": "state", "playing": true, "levels": [-6.0]}).to_string()
}

fn audio() -> Vec<u8> {
    let mut f = b"RTSL".to_vec();
    f.extend([0u8; 28]);
    f
}

#[test]
fn only_superseded_frames_are_droppable() {
    assert!(droppable_engine_frame(state().as_bytes(), false));
    assert!(droppable_engine_frame(&audio(), true));
    // A reply that merely says the word, a nested `type`, and a binary frame of another kind all go.
    let reply = json!({"type": "applied", "note": "state"}).to_string();
    for text in [
        reply.as_str(),
        r#"{"type":"sessions","list":[{"type":"state"}]}"#,
        "not json \"state\"",
    ] {
        assert!(!droppable_engine_frame(text.as_bytes(), false), "{text}");
    }
    assert!(!droppable_engine_frame(b"PK\x03\x04", true) && !droppable_engine_frame(b"RT", true));
}

#[test]
fn a_frame_is_dropped_only_past_the_mark_and_the_count_waits_for_the_peer() {
    let mut shed = FrameShedder::default();
    assert!(shed.admit(state().as_bytes(), false, DROP_ABOVE_BYTES));
    assert_eq!(shed.take_shed(), 0);
    assert!(!shed.admit(state().as_bytes(), false, DROP_ABOVE_BYTES + 1));
    assert!(!shed.admit(&audio(), true, DROP_ABOVE_BYTES + 1));
    // A reply is never dropped, and the count waits for the peer to catch up.
    assert!(shed.admit(br#"{"type":"applied"}"#, false, DROP_ABOVE_BYTES + 1));
    assert_eq!(shed.take_shed(), 0);
    assert!(shed.admit(state().as_bytes(), false, 0));
    assert_eq!((shed.take_shed(), shed.take_shed()), (2, 0));
}

#[tokio::test]
async fn a_sender_goes_through_at_the_mark_and_waits_above_it_until_the_buffer_drains() {
    let link = Arc::new(Link::default());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    link.attach(tx);
    assert_eq!(HIGH_WATER_BYTES, 1024 * 1024);
    link.send_binary(vec![0; HIGH_WATER_BYTES]);
    link.wait_below(HIGH_WATER_BYTES).await;
    link.send_binary(vec![0]);
    let done = Arc::new(AtomicUsize::new(0));
    let waiting = {
        let (link, done) = (link.clone(), done.clone());
        tokio::spawn(async move {
            link.wait_below(HIGH_WATER_BYTES).await;
            done.store(1, Ordering::SeqCst);
        })
    };
    tokio::time::sleep(MS(120)).await;
    assert_eq!(done.load(Ordering::SeqCst), 0);
    link.written(HIGH_WATER_BYTES + 1);
    waiting.await.unwrap();
    assert_eq!(done.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_sender_stops_waiting_on_a_link_that_went_down() {
    let link = Link::default();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    link.attach(tx);
    link.send_binary(vec![0; 2 * HIGH_WATER_BYTES]);
    link.detach();
    link.wait_below(HIGH_WATER_BYTES).await;
}

#[test]
fn a_backlog_that_never_clears_is_still_reported() {
    let mut shed = FrameShedder::default();
    let refused = (0..SHED_REPORT_EVERY)
        .filter(|_| !shed.admit(state().as_bytes(), false, 10 * DROP_ABOVE_BYTES))
        .count() as u64;
    assert_eq!(
        (refused, shed.take_shed()),
        (SHED_REPORT_EVERY, SHED_REPORT_EVERY)
    );
}
