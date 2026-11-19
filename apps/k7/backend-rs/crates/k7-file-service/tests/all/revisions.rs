//! The token's memo over a fake feed (1310 to 1325): equal inputs answer one token, the memo stands only while the
//! feed vouches, a write or a rename moves it, a lost event is found by the ceiling, and no epoch repeats.

use crate::support;

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use k7_file_service::revisions::{MAX_WATCHED, Revisions};
use k7_file_service::watch::Feed;
use support::Scratch;

thread_local! {
    static OFFSET: Cell<u64> = const { Cell::new(0) };
}

/// A clock each test moves by hand; thread-local, so two tests never share a second.
fn clock() -> Instant {
    static BASE: OnceLock<Instant> = OnceLock::new();
    *BASE.get_or_init(Instant::now) + Duration::from_secs(OFFSET.with(Cell::get))
}

fn advance(secs: u64) {
    OFFSET.with(|o| o.set(o.get() + secs));
}

/// A feed the test moves: its generation, whether it is up, and whether the next read is followed by one event.
#[derive(Default)]
struct Fake {
    tick: AtomicU64,
    down: AtomicBool,
    chatty: AtomicBool,
}

struct Handle(Arc<Fake>);

impl Feed for Handle {
    fn generation(&self, _root: &str) -> Option<u64> {
        if self.0.down.load(Ordering::Relaxed) {
            return None;
        }
        let g = self.0.tick.load(Ordering::Relaxed);
        if self.0.chatty.swap(false, Ordering::Relaxed) {
            self.0.tick.fetch_add(1, Ordering::Relaxed);
        }
        Some(g)
    }
}

fn memo() -> (Revisions, Arc<Fake>) {
    OFFSET.with(|o| o.set(0));
    let f = Arc::new(Fake::default());
    (Revisions::with_feed(Box::new(Handle(f.clone())), clock), f)
}

fn project(name: &str) -> (Scratch, String, String) {
    let s = Scratch::new(name);
    s.file("ws/p.k7/main.k7x", "a");
    let (root, dir) = (s.path("ws"), s.path("ws/p.k7"));
    (s, root, dir)
}

#[test]
fn equal_inputs_answer_one_token_and_the_memo_takes_no_walk() {
    // 1310, 1320
    let (s, root, dir) = project("revs-same");
    let (m, _f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    for _ in 0..5 {
        assert_eq!(m.token(&h, &root, Some(&dir), false), first);
    }
    assert_eq!(m.walks(), 1);
    assert!(!first.starts_with("none:") && first.contains('.'));
}

#[test]
fn a_moved_feed_walks_and_a_changed_project_takes_a_new_token() {
    let (s, root, dir) = project("revs-moved");
    let (m, f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    s.file("ws/p.k7/graph.k7x", "b");
    // Nothing heard: the memo answers, and the edit waits for the feed or the ceiling.
    assert_eq!(m.token(&h, &root, Some(&dir), false), first);
    f.tick.fetch_add(1, Ordering::Relaxed);
    let second = m.token(&h, &root, Some(&dir), false);
    assert_ne!(second, first);
    assert_eq!(m.token(&h, &root, Some(&dir), false), second);
    // A walk that finds the same state answers the same token.
    f.tick.fetch_add(1, Ordering::Relaxed);
    assert_eq!(m.token(&h, &root, Some(&dir), false), second);
    assert_eq!(m.walks(), 3);
}

#[test]
fn a_rename_moves_the_token() {
    // 1311
    let (s, root, dir) = project("revs-rename");
    let (m, f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    std::fs::rename(s.path("ws/p.k7/main.k7x"), s.path("ws/p.k7/other.k7x")).unwrap();
    f.tick.fetch_add(1, Ordering::Relaxed);
    assert_ne!(m.token(&h, &root, Some(&dir), false), first);
}

#[test]
fn a_bump_moves_the_token_and_the_next_ask_walks() {
    // 1306
    let (s, root, dir) = project("revs-bump");
    let (m, _f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    m.bump(&root);
    let second = m.token(&h, &root, Some(&dir), false);
    assert_ne!(second, first);
    assert_eq!(m.walks(), 2);
}

#[test]
fn a_fresh_ask_walks_and_never_answers_from_the_memo() {
    // 1305: an `after` taken inside a mutation sees the write the watcher has not reported yet
    let (s, root, dir) = project("revs-fresh");
    let (m, _f) = memo();
    let h = s.home();
    let before = m.token(&h, &root, Some(&dir), true);
    s.file("ws/p.k7/graph.k7x", "b");
    let after = m.token(&h, &root, Some(&dir), true);
    assert_ne!(after, before);
    assert_eq!(m.walks(), 2);
}

#[test]
fn no_active_project_is_none_and_follows_the_mutations() {
    let (s, root, _dir) = project("revs-none");
    let (m, _f) = memo();
    let h = s.home();
    let a = m.token(&h, &root, None, false);
    assert!(a.starts_with("none:"));
    assert_eq!(m.token(&h, &root, None, false), a);
    m.bump(&root);
    assert_ne!(m.token(&h, &root, None, false), a);
    assert_eq!(m.walks(), 0);
}

#[test]
fn an_event_landing_during_the_walk_is_not_absorbed() {
    // 1315: the generation is read before the walk; read after it, this feed's event would be forgotten
    let (s, root, dir) = project("revs-midwalk");
    let (m, f) = memo();
    f.chatty.store(true, Ordering::Relaxed);
    let h = s.home();
    m.token(&h, &root, Some(&dir), false);
    m.token(&h, &root, Some(&dir), false);
    assert_eq!(m.walks(), 2);
}

#[test]
fn a_feed_that_loses_every_event_is_found_by_the_ceiling() {
    // 1321: moved within 10 s, `missed` counted
    let (s, root, dir) = project("revs-lossy");
    let (m, _f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    s.file("ws/p.k7/graph.k7x", "b");
    advance(5);
    assert_eq!(m.token(&h, &root, Some(&dir), false), first);
    advance(6);
    let found = m.token(&h, &root, Some(&dir), false);
    assert_ne!(found, first);
    assert_eq!(m.missed(), 1);
    // A ceiling walk that finds nothing is no loss.
    advance(11);
    assert_eq!(m.token(&h, &root, Some(&dir), false), found);
    assert_eq!(m.missed(), 1);
}

#[test]
fn a_feed_that_is_down_walks_at_every_ask() {
    // 1320
    let (s, root, dir) = project("revs-down");
    let (m, f) = memo();
    f.down.store(true, Ordering::Relaxed);
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    for _ in 0..4 {
        assert_eq!(m.token(&h, &root, Some(&dir), false), first);
    }
    assert_eq!(m.walks(), 5);
}

#[test]
fn another_project_walks_and_a_walk_past_a_limit_never_repeats() {
    // 1314: a nest past the depth limit is unknowable and takes a new epoch at every ask
    let s = Scratch::new("revs-deep");
    s.file("ws/a.k7/x.k7x", "");
    s.file(&format!("ws/deep.k7/{}x.k7x", "d/".repeat(300)), "");
    let (root, a, deep) = (s.path("ws"), s.path("ws/a.k7"), s.path("ws/deep.k7"));
    let (m, _f) = memo();
    let h = s.home();
    let ta = m.token(&h, &root, Some(&a), false);
    let t1 = m.token(&h, &root, Some(&deep), false);
    let t2 = m.token(&h, &root, Some(&deep), false);
    assert!(ta != t1 && t1 != t2);
    // The root keeps one last state: a project left and come back to is a new state, never an old token.
    let back = m.token(&h, &root, Some(&a), false);
    assert!(back != ta && back != t2);
}

#[test]
fn a_thousand_asks_from_eight_threads_make_one_walk_and_one_token() {
    // 1315
    let (s, root, dir) = project("revs-threads");
    let (m, _f) = memo();
    let h = s.home();
    let tokens: Vec<Vec<String>> = std::thread::scope(|scope| {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    (0..125)
                        .map(|_| m.token(&h, &root, Some(&dir), false))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().map(|j| j.join().unwrap()).collect()
    });
    let first = &tokens[0][0];
    assert!(tokens.iter().flatten().all(|t| t == first));
    assert_eq!(m.walks(), 1);
}

#[test]
fn sixty_five_roots_asked_in_turn_leave_the_oldest_unwatched() {
    // 1325
    let s = Scratch::new("revs-many");
    let (m, _f) = memo();
    let h = s.home();
    let mut tokens = Vec::new();
    let mut roots = Vec::new();
    for i in 0..=MAX_WATCHED {
        s.file(&format!("w{i}/p.k7/x.k7x"), "");
        roots.push(s.path(&format!("w{i}")));
        tokens.push(m.token(&h, &roots[i], Some(&s.path(&format!("w{i}/p.k7"))), false));
        advance(1);
    }
    assert_eq!(m.watched(), MAX_WATCHED);
    assert!(!m.is_watched(&roots[0]) && m.is_watched(&roots[MAX_WATCHED]));
    let again = m.token(&h, &roots[0], Some(&s.path("w0/p.k7")), false);
    // Unchanged inputs keep the token; the root is watched again and walked once more.
    assert_eq!(again, tokens[0]);
    assert_eq!(m.walks(), 66);
    let mut all = tokens.clone();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), tokens.len());
}

#[test]
fn a_root_idle_for_fifteen_minutes_is_unwatched() {
    let s = Scratch::new("revs-idle");
    s.file("a/p.k7/x.k7x", "");
    s.file("b/p.k7/x.k7x", "");
    let (m, _f) = memo();
    let h = s.home();
    m.token(&h, &s.path("a"), Some(&s.path("a/p.k7")), false);
    advance(16 * 60);
    m.token(&h, &s.path("b"), Some(&s.path("b/p.k7")), false);
    assert!(!m.is_watched(&s.path("a")) && m.is_watched(&s.path("b")));
}

#[test]
fn a_forgotten_root_asked_again_takes_a_token_never_given_before() {
    // 1325: the epoch is the process's
    let (s, root, dir) = project("revs-forget");
    let (m, _f) = memo();
    let h = s.home();
    let first = m.token(&h, &root, Some(&dir), false);
    m.forget(&root);
    assert!(!m.is_watched(&root));
    let second = m.token(&h, &root, Some(&dir), false);
    assert_ne!(second, first);
}

#[test]
fn a_removal_forgets_every_root_below_the_account_s_workspaces_and_no_other() {
    // 1342: the front does not read which workspace the body names
    let s = Scratch::new("revs-forget-under");
    for ws in ["home/workspace/a", "home/workspace/b", "other/c"] {
        s.file(&format!("{ws}/p.k7/x.k7x"), "");
    }
    let (m, _f) = memo();
    let h = s.home();
    let ask = |ws: &str| m.token(&h, &s.path(ws), Some(&s.path(&format!("{ws}/p.k7"))), false);
    let before: Vec<String> = ["home/workspace/a", "home/workspace/b", "other/c"]
        .map(ask)
        .to_vec();
    m.forget_under(&s.path("home/workspace"));
    assert!(
        !m.is_watched(&s.path("home/workspace/a")) && !m.is_watched(&s.path("home/workspace/b"))
    );
    assert!(m.is_watched(&s.path("other/c")));
    assert_ne!(ask("home/workspace/a"), before[0]);
    assert_eq!(ask("other/c"), before[2]);
}
