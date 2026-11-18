//! The real watcher on this host (1322 to 1324): another process's write moves the token within the watcher's
//! latency, a rename and a new folder with files do, a take's media does not, and the walk's own reads do not.

use crate::support;

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k7_file_service::revisions::Revisions;
use k7_file_service::watch::{Feed, Watch};
use support::Scratch;

/// Generous on a loaded host: the latency is printed, only the bound is asserted.
const BOUND: Duration = Duration::from_secs(30);

fn rig(name: &str) -> (Scratch, String, String, &'static Watch, Revisions) {
    let s = Scratch::new(name);
    s.file("ws/p.k7/main.k7x", "a");
    let (root, dir) = (s.path("ws"), s.path("ws/p.k7"));
    let w: &'static Watch = Box::leak(Box::new(Watch::new()));
    let m = Revisions::with_watch(w, Instant::now);
    (s, root, dir, w, m)
}

/// A clock that never moves: no ceiling walk, so only an event can move the token.
fn frozen() -> Instant {
    static AT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *AT.get_or_init(Instant::now)
}

/// A write by a process of its own, which no handle of this one made: this test binary run as a child.
fn write_elsewhere(path: &str, text: &str) {
    let ok = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "watching::child_writes", "--nocapture"])
        .env("K7_WATCH_CHILD", format!("{path}\n{text}"))
        .status();
    assert!(ok.unwrap().success());
}

/// The child of `write_elsewhere`: a test that does nothing unless it was spawned to write.
#[test]
fn child_writes() {
    if let Ok(job) = std::env::var("K7_WATCH_CHILD")
        && let Some((path, text)) = job.split_once('\n')
    {
        std::fs::write(path, text).unwrap();
    }
}

/// The first token after `from` that differs from `was`, and how long it took.
fn until_moved(m: &Revisions, s: &Scratch, root: &str, dir: &str, was: &str) -> Option<Duration> {
    let t0 = Instant::now();
    while t0.elapsed() < BOUND {
        if m.token(&s.home(false), root, Some(dir), false) != was {
            return Some(t0.elapsed());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

/// Waits for a canary write to move the token (the stream is live), then for a quiet second; polls up to `BOUND`.
fn settle(m: &Revisions, s: &Scratch, root: &str, dir: &str) -> String {
    let by = Instant::now() + BOUND;
    let was = m.token(&s.home(false), root, Some(dir), false);
    std::fs::write(format!("{dir}/settle-canary.k7x"), "canary").unwrap();
    while Instant::now() < by {
        if m.token(&s.home(false), root, Some(dir), false) != was {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut last = m.token(&s.home(false), root, Some(dir), false);
    while Instant::now() < by {
        std::thread::sleep(Duration::from_millis(50));
        let now = m.token(&s.home(false), root, Some(dir), false);
        if now == last && m.walks() > 0 {
            let walks = m.walks();
            std::thread::sleep(Duration::from_secs(1));
            if m.token(&s.home(false), root, Some(dir), false) == now && m.walks() == walks {
                return now;
            }
        }
        last = now;
    }
    last
}

#[test]
fn another_process_s_write_moves_the_token_within_the_latency() {
    let (s, root, dir, _w, m) = rig("watch-write");
    let was = settle(&m, &s, &root, &dir);
    let t0 = Instant::now();
    write_elsewhere(&s.path("ws/p.k7/graph.k7x"), "b");
    let moved = until_moved(&m, &s, &root, &dir, &was).expect("the token never moved");
    println!(
        "write by another process to a moved token: {moved:?} (since the spawn: {:?})",
        t0.elapsed()
    );
}

#[test]
fn a_rename_moves_the_token() {
    let (s, root, dir, _w, m) = rig("watch-rename");
    let was = settle(&m, &s, &root, &dir);
    std::fs::rename(s.path("ws/p.k7/main.k7x"), s.path("ws/p.k7/moved.k7x")).unwrap();
    assert!(until_moved(&m, &s, &root, &dir, &was).is_some());
}

#[test]
fn a_folder_made_elsewhere_with_files_inside_and_renamed_in_moves_the_token() {
    let (s, root, dir, _w, m) = rig("watch-folder");
    s.file("outside/sub/inner.k7x", "x");
    let was = settle(&m, &s, &root, &dir);
    std::fs::rename(s.path("outside/sub"), s.path("ws/p.k7/sub")).unwrap();
    assert!(until_moved(&m, &s, &root, &dir, &was).is_some());
}

#[test]
fn a_burst_of_two_thousand_files_moves_the_token() {
    // On Linux and Windows the burst may overflow; the ceiling then moves it, within the bound only on the host's word
    let (s, root, dir, _w, m) = rig("watch-burst");
    let was = settle(&m, &s, &root, &dir);
    let sub = s.dir("ws/p.k7/burst");
    for i in 0..2000 {
        std::fs::write(format!("{sub}/f{i}.k7x"), "").unwrap();
    }
    assert!(until_moved(&m, &s, &root, &dir, &was).is_some());
}

#[test]
fn a_takes_media_does_not_move_the_token() {
    // 1324: a recording into clips/ must not make every poll walk
    let (s, root, dir, _w, m) = rig("watch-media");
    let clips = s.dir("ws/p.k7/clips");
    let was = settle(&m, &s, &root, &dir);
    let walks = m.walks();
    for i in 0..20 {
        std::fs::write(format!("{clips}/take{i}.wav"), "riff").unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(m.token(&s.home(false), &root, Some(&dir), false), was);
    assert_eq!(m.walks(), walks);
}

#[test]
fn the_walk_does_not_dirty_its_own_memo() {
    // 1324: inotify reports each readdir as an open; asks over a settled root take no walk
    let (s, root, dir, _w, m) = rig("watch-own-reads");
    s.file("ws/p.k7/lanes/l.k7x", "");
    let was = settle(&m, &s, &root, &dir);
    // The first ask after the settle may still take the walk that keeps the token; it is not counted.
    m.token(&s.home(false), &root, Some(&dir), false);
    let walks = m.walks();
    for _ in 0..20 {
        assert_eq!(m.token(&s.home(false), &root, Some(&dir), false), was);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(m.walks(), walks);
}

#[test]
fn a_root_removed_while_watched_is_no_panic_and_walks_afterwards() {
    let (s, root, dir, w, m) = rig("watch-removed");
    settle(&m, &s, &root, &dir);
    // Windows refuses to remove a folder above a watched one: the watch is dropped first (1323).
    w.unwatch(&root);
    std::fs::remove_dir_all(s.path("ws")).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let t = m.token(&s.home(false), &root, Some(&dir), false);
    assert!(!t.is_empty());
    assert!(w.generation(&root).is_none() || w.watched() <= 1);
}

#[test]
fn the_event_filter_table() {
    use k7_file_service::watch::relevant;
    use notify::EventKind;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode};
    let any = EventKind::Modify(ModifyKind::Any);
    let no = || Some(false);
    let gone = || None;
    // 1324: an open is the walk's own readdir; nothing else about it matters
    assert!(!relevant(&EventKind::Access(AccessKind::Any), "a.k7x", no));
    assert!(relevant(&any, "", no), "the root itself");
    assert!(relevant(&any, "a.k7x", no) && relevant(&any, "d/B.VSTPRESET", no));
    assert!(relevant(&any, "song.k7", no), "a project folder");
    assert!(
        relevant(&any, "lanes", no),
        "no extension: a folder or a file we cannot tell"
    );
    assert!(!relevant(&any, "clips/take.wav", no));
    // 1324: ReadDirectoryChangesW says `clips` was modified when a take lands in it; that must not move the token.
    let folder = || Some(true);
    assert!(
        !relevant(&any, "clips", folder),
        "a folder's own modification"
    );
    assert!(!relevant(&any, "d/clips", folder));
    assert!(
        relevant(&any, "clips", no),
        "a file without an extension is unknown"
    );
    assert!(
        relevant(
            &EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            "clips",
            folder
        ),
        "a rename is not"
    );
    assert!(relevant(
        &EventKind::Create(CreateKind::Folder),
        "clips",
        folder
    ));
    assert!(!relevant(&any, "notes.md", no));
    assert!(!relevant(&any, ".sync/x.k7x", no) && !relevant(&any, "d/node_modules/x.k7x", no));
    assert!(!relevant(&any, "a.cache/y.k7x", no));
    assert!(relevant(&EventKind::Create(CreateKind::Folder), "v1.2", no));
    // 1344: a rename or an untyped removal of a path that is gone may have taken a folder of sources with it.
    let from = EventKind::Modify(ModifyKind::Name(RenameMode::From));
    assert!(
        relevant(&from, "v1.2", gone)
            && relevant(&EventKind::Remove(RemoveKind::Any), "v1.2", gone)
    );
    assert!(
        !relevant(&from, "take.wav", no),
        "a file renamed in place is a file"
    );
    assert!(!relevant(
        &EventKind::Remove(RemoveKind::File),
        "take.wav",
        gone
    ));
    assert!(!relevant(&any, "take.wav", gone));
    assert!(
        !relevant(&any, "v1.2", folder),
        "a folder's own modification, whatever its name"
    );
}

#[test]
fn a_dotted_folder_moved_out_of_the_root_moves_the_token() {
    // 1344: the rename's old path is gone, so nothing says it was a folder; the clock is frozen, so no ceiling helps
    let s = Scratch::new("watch-moved-out");
    s.file("ws/p.k7/parts.v1/a.k7x", "a");
    s.dir("away");
    let (root, dir) = (s.path("ws"), s.path("ws/p.k7"));
    let w: &'static Watch = Box::leak(Box::new(Watch::new()));
    let m = Revisions::with_watch(w, frozen);
    let was = settle(&m, &s, &root, &dir);
    std::fs::rename(s.path("ws/p.k7/parts.v1"), s.path("away/parts.v1")).unwrap();
    assert!(until_moved(&m, &s, &root, &dir, &was).is_some());
}

#[test]
fn watching_or_unwatching_a_root_moves_every_other_root() {
    // 1345: FSEvents restarts its one stream for each root added or dropped, and what fell between is lost
    let s = Scratch::new("watch-restart");
    let (a, b) = (s.dir("a"), s.dir("b"));
    let w = Watch::new();
    w.watch(&a);
    let g = w.generation(&a).expect("a is watched");
    w.watch(&b);
    let after_watch = w.generation(&a).expect("a is watched");
    assert_ne!(after_watch, g);
    w.unwatch(&b);
    assert_ne!(w.generation(&a).expect("a is watched"), after_watch);
}

/// Paths a pinned root's sink heard, in order.
fn listen() -> (k7_file_service::watch::Sink, Arc<Mutex<Vec<String>>>) {
    let heard = Arc::new(Mutex::new(Vec::<String>::new()));
    let into = heard.clone();
    let sink: k7_file_service::watch::Sink =
        Arc::new(move |rels| into.lock().unwrap().extend(rels));
    (sink, heard)
}

/// Writes `file` again every so often until `heard` names it: the first events of a new watch may fall before it is up.
fn written_until_heard(file: &str, rel: &str, heard: &Arc<Mutex<Vec<String>>>) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < BOUND {
        std::fs::write(file, t0.elapsed().as_nanos().to_string()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        if heard.lock().unwrap().iter().any(|h| h == rel) {
            return true;
        }
    }
    false
}

#[test]
fn a_pinned_root_hears_every_path_survives_an_eviction_and_ends_at_unpin() {
    let s = Scratch::new("watch-pin");
    s.file("ws/seed.txt", "s");
    let root = s.path("ws");
    let w = Watch::new();
    let (sink, heard) = listen();
    assert!(w.pin(&root, sink.clone()));
    assert_eq!(w.watched(), 1);
    // A take's media moves no token, but a capture wants it.
    s.dir("ws/clips");
    assert!(written_until_heard(
        &s.path("ws/clips/take.wav"),
        "clips/take.wav",
        &heard
    ));
    // The revision's eviction (an idle root, the 64th) asks to unwatch; a pinned root stays.
    w.unwatch(&root);
    assert_eq!(w.watched(), 1);
    assert!(written_until_heard(
        &s.path("ws/after.md"),
        "after.md",
        &heard
    ));
    // Unpinned before its folder is renamed or removed: nothing is heard, nothing is watched.
    w.unpin(&root, &sink);
    assert_eq!(w.watched(), 0);
    heard.lock().unwrap().clear();
    std::fs::write(s.path("ws/late.md"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(heard.lock().unwrap().is_empty());
}

#[test]
fn a_root_that_is_not_pinned_is_dropped_by_unwatch_and_unpin_leaves_it() {
    let s = Scratch::new("watch-unpinned");
    let root = s.dir("ws");
    let w = Watch::new();
    w.watch(&root);
    assert_eq!(w.watched(), 1);
    w.unpin(&root, &listen().0);
    assert_eq!(w.watched(), 1);
    w.unwatch(&root);
    assert_eq!(w.watched(), 0);
}

#[test]
fn an_unpin_by_an_old_sink_leaves_the_root_pinned_again_by_a_newer_one() {
    let s = Scratch::new("watch-repin");
    let root = s.dir("ws");
    let w = Watch::new();
    let (old, _) = listen();
    let (new, heard) = listen();
    assert!(w.pin(&root, old.clone()));
    assert!(w.pin(&root, new.clone()));
    w.unpin(&root, &old);
    assert_eq!(w.watched(), 1);
    assert!(written_until_heard(&s.path("ws/a.md"), "a.md", &heard));
    w.unpin(&root, &new);
    assert_eq!(w.watched(), 0);
}

#[test]
fn a_pinned_root_watched_again_moves_its_generation_and_keeps_its_sink() {
    // 1c's recovery from a missed change: unwatch and watch are no-ops on a pinned root, so it asks for `rewatch`
    let s = Scratch::new("watch-rewatch");
    let root = s.dir("ws");
    let w = Watch::new();
    let (sink, heard) = listen();
    assert!(w.pin(&root, sink));
    let g = w.generation(&root).expect("pinned");
    w.rewatch(&root);
    assert_ne!(w.generation(&root).expect("still watched"), g);
    assert_eq!(w.watched(), 1);
    assert!(written_until_heard(&s.path("ws/b.md"), "b.md", &heard));
}
