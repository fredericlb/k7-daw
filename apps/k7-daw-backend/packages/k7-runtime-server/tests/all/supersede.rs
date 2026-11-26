//! supersede.rs — which queued jobs a newer message skips and which it must leave alone, and the notes a socket
//! that drops mid-note leaves to release: the behaviours of `Supersede_test.cpp` and `HeldNotes_test.cpp`.

use k7_runtime_server::server::held_notes::{HeldNotes, MAX_TRACKS_PER_SOCKET};
use k7_runtime_server::server::supersede::{Supersede, Ticket, superseded};
use std::cell::RefCell;

/// The message queue in posting order: a ticket for a job that may be skipped, none for one that never is.
#[derive(Default)]
struct Queue(RefCell<Vec<(Option<Ticket>, &'static str)>>);

impl Queue {
    fn push(&self, t: Ticket, tag: &'static str) {
        self.0.borrow_mut().push((Some(t), tag));
    }
    fn project(&self, l: &Supersede, s: &str, tag: &'static str) {
        l.post_project(s, |t| self.push(t, tag));
    }
    fn tracks(&self, l: &Supersede, s: &str, ids: &[&str], tag: &'static str) {
        l.post_tracks(s, ids.iter().map(|i| (*i).to_owned()).collect(), |t| {
            self.push(t, tag)
        });
    }
    fn param(&self, l: &Supersede, s: &str, plugin: &str, name: &str, tag: &'static str) {
        l.post_param(s, plugin, name, |t| self.push(t, tag));
    }
    /// A verb that is never superseded: the dispatcher fences, then the handler posts.
    fn other(&self, l: &Supersede, s: &str, tag: &'static str) {
        l.barrier(s);
        self.0.borrow_mut().push((None, tag));
    }
    /// What ran, in posting order.
    fn drain(&self) -> Vec<&'static str> {
        let jobs = self.0.take();
        jobs.into_iter()
            .filter(|(t, _)| t.as_ref().is_none_or(|t| !superseded(t)))
            .map(|(_, tag)| tag)
            .collect()
    }
}

#[test]
fn the_latest_project_wins() {
    let (l, q) = (Supersede::default(), Queue::default());
    for tag in ["p1", "p2", "p3"] {
        q.project(&l, "k7", tag);
    }
    assert_eq!(q.drain(), ["p3"]);
}

#[test]
fn another_verb_fences() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.project(&l, "k7", "p1");
    q.other(&l, "k7", "play");
    q.project(&l, "k7", "p2");
    assert_eq!(q.drain(), ["p1", "play", "p2"]);
}

#[test]
fn sessions_are_apart() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.project(&l, "a", "a1");
    q.project(&l, "b", "b1");
    q.other(&l, "b", "b-play");
    q.project(&l, "a", "a2");
    assert_eq!(q.drain(), ["b1", "b-play", "a2"]);
}

#[test]
fn a_project_swallows_an_earlier_delta_and_a_delta_never_swallows_a_project() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.tracks(&l, "k7", &["drums"], "t1");
    q.project(&l, "k7", "p1");
    q.tracks(&l, "k7", &["drums"], "t2");
    assert_eq!(q.drain(), ["p1", "t2"]);
    q.project(&l, "k7", "p1");
    q.tracks(&l, "k7", &["drums"], "t1");
    assert_eq!(q.drain(), ["p1", "t1"]);
}

#[test]
fn a_delta_overtakes_only_one_naming_the_same_set_of_tracks() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.tracks(&l, "k7", &["bass", "drums"], "t1");
    q.tracks(&l, "k7", &["keys"], "t2");
    q.tracks(&l, "k7", &["drums", "bass", "drums"], "t3");
    assert_eq!(q.drain(), ["t2", "t3"]);
}

#[test]
fn a_parameter_is_superseded_by_the_same_plugins_same_name_and_coalesces_with_documents() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.param(&l, "k7", "synth", "cutoff", "c1");
    q.param(&l, "k7", "synth", "res", "r1");
    q.param(&l, "k7", "delay", "cutoff", "d1");
    q.param(&l, "k7", "synth", "cutoff", "c2");
    assert_eq!(q.drain(), ["r1", "d1", "c2"]);
    q.project(&l, "k7", "p1");
    q.param(&l, "k7", "synth", "cutoff", "c1");
    q.project(&l, "k7", "p2");
    q.param(&l, "k7", "synth", "cutoff", "c2");
    assert_eq!(q.drain(), ["p2", "c2"]);
}

#[test]
fn a_barrier_fences_parameters_too_and_a_job_that_ran_stays_run() {
    let (l, q) = (Supersede::default(), Queue::default());
    q.param(&l, "k7", "synth", "cutoff", "c1");
    q.other(&l, "k7", "set-preset");
    q.param(&l, "k7", "synth", "cutoff", "c2");
    assert_eq!(q.drain(), ["c1", "set-preset", "c2"]);
    q.project(&l, "k7", "p1");
    assert_eq!(q.drain(), ["p1"]);
    q.project(&l, "k7", "p2");
    assert_eq!(q.drain(), ["p2"]);
    assert_eq!(l.session_count(), 1);
    l.barrier("k7");
    assert_eq!(l.session_count(), 0);
}

const A: u64 = 1;
const B: u64 = 2;

#[test]
fn a_note_released_by_the_client_is_not_released_again() {
    let h = HeldNotes::default();
    h.sent(A, "s", "keys", &[0x90, 60, 100]);
    h.sent(A, "s", "keys", &[0x80, 60, 0]);
    h.sent(A, "s", "keys", &[0x91, 62, 90]);
    // A velocity of 0 is a note-off.
    h.sent(A, "s", "keys", &[0x90, 62, 0]);
    assert!(h.closed(A).is_empty());
}

#[test]
fn what_is_held_is_released_on_close_and_forgotten() {
    let h = HeldNotes::default();
    h.sent(A, "s", "keys", &[0x90, 60, 100]);
    h.sent(A, "s", "keys", &[0x90, 64, 100]);
    h.sent(A, "s", "keys", &[0xB0, 64, 127]);
    h.sent(A, "s", "bass", &[0x90, 40, 100]);
    h.sent(B, "s", "keys", &[0x90, 67, 100]);
    let out = h.closed(A);
    assert_eq!(out.len(), 2);
    let keys = out.iter().find(|r| r.track == "keys").unwrap();
    assert_eq!(
        keys.messages,
        [vec![0x80, 60, 0], vec![0x80, 64, 0], vec![0xB0, 64, 0]]
    );
    assert_eq!(
        out.iter()
            .find(|r| r.track == "bass")
            .unwrap()
            .messages
            .len(),
        1
    );
    assert!(h.closed(A).is_empty());
    assert_eq!(h.closed(B).len(), 1);
}

#[test]
fn all_notes_off_and_pedal_up_leave_nothing_and_the_tracks_followed_are_bounded() {
    let h = HeldNotes::default();
    for m in [
        [0x90, 60, 100],
        [0xB0, 64, 127],
        [0xB0, 123, 0],
        [0xB0, 64, 0],
    ] {
        h.sent(A, "s", "keys", &m);
    }
    assert!(h.closed(A).is_empty());
    for i in 0..MAX_TRACKS_PER_SOCKET + 10 {
        h.sent(A, "s", &format!("t{i}"), &[0x90, 60, 100]);
    }
    assert_eq!(h.closed(A).len(), MAX_TRACKS_PER_SOCKET);
}
