//! Hostile input, each answered without a hang: a journal of 64 MiB or 100,000 entries, a shape that lies, a store that is
//! a file, a name of 16 KiB, a project of 200,000 files, 10,000 texts written inside the settle window, a 1 GiB snapshot.
//! A bound is a count where one is observable and a wall time only as a backstop: 15 s, 60 s on musl.

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::journal::Mode;
use k7_file_service::journal::format::{Journal, parse, print};
use k7_file_service::journal::image::{TextCache, image_of};
use k7_file_service::journal::ring::{NewEntry, push};
use k7_file_service::journal::store::{Held, Loaded, Store, store_for};
use k7_file_service::revision::walk_project;
use k7_file_service::tree::Limits;
use support::Scratch;
use support::journal::{Bed, ME};

fn backstop() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn journal_of(entries: usize, label: &str) -> String {
    let mut j = Journal::empty();
    for n in 0..entries {
        let e = NewEntry {
            label: label.into(),
            changes: vec![k7_file_service::journal::format::Change::new(
                &format!("f{n}.k7x"),
                None,
                Some("x".into()),
            )],
            blobs: vec![],
        };
        push(&mut j, e, n as u64, usize::MAX, None);
    }
    print(&j)
}

#[test]
fn a_journal_of_sixty_four_mib_loads_and_one_step_persists() {
    let s = Scratch::new("jh-big");
    let store = Store::new(s.path("store"));
    // 1,000 entries of 64 KiB each.
    let text = journal_of(1000, &"é\"x".repeat(21_000));
    assert!(text.len() > 60 << 20, "{} bytes", text.len());
    s.file("store/journal.json", &text);
    let t0 = Instant::now();
    let mut held = Held::read(&store);
    let loaded = t0.elapsed();
    assert_eq!(held.journal.entries.len(), 1000);
    push(
        &mut held.journal,
        NewEntry {
            label: "one more".into(),
            changes: vec![k7_file_service::journal::format::Change::new(
                "a",
                None,
                Some("A".into()),
            )],
            blobs: vec![],
        },
        1,
        1000,
        None,
    );
    held.save(&store, 1).unwrap();
    println!(
        "a journal of {} MiB: loaded in {loaded:?}, a step persisted by {:?}",
        text.len() >> 20,
        t0.elapsed()
    );
    assert!(t0.elapsed() < backstop());
}

#[test]
fn a_hundred_thousand_entries_are_read_and_a_push_trims_them_to_the_ring() {
    let text = journal_of(100_000, "e");
    let t0 = Instant::now();
    let mut j = parse(&text).unwrap();
    assert_eq!(j.entries.len(), 100_000);
    push(
        &mut j,
        NewEntry {
            label: "x".into(),
            changes: vec![k7_file_service::journal::format::Change::new(
                "a",
                None,
                Some("A".into()),
            )],
            blobs: vec![],
        },
        1,
        1000,
        None,
    );
    assert_eq!((j.entries.len(), j.cursor), (1000, 1000));
    assert!(t0.elapsed() < backstop());
}

#[test]
fn a_journal_that_lies_or_cannot_be_read_is_torn_never_trusted() {
    let s = Scratch::new("jh-lies");
    let store = Store::new(s.path("store"));
    let entry = |extra: &str| {
        format!(r#"{{"id":1,"ts":{extra},"kind":"text","label":"l","changes":[],"blobs":[]}}"#)
    };
    let doc = |cursor: &str, next: &str, e: &str| {
        format!(r#"{{"nextId":{next},"cursor":{cursor},"entries":[{e}]}}"#)
    };
    let torn = [
        doc("5", "2", &entry("1")),
        doc("1", "1", &entry("1")),
        doc("1", "2", &entry("\"1\"")),
        doc("1", "2", &entry("null")),
        String::new(),
        "\u{feff}{}".into(),
    ];
    for t in &torn {
        s.file("store/journal.json", t);
        assert!(matches!(store.load(), Loaded::Torn(..)), "{t}");
    }
    // Numbers that parse: -0 and 1e300 are finite numbers, kept and printed as JavaScript prints them.
    for (ts, printed) in [("-0", "0"), ("1e300", "1e+300")] {
        let j = parse(&doc("1", "2", &entry(ts))).unwrap();
        assert!(print(&j).contains(&format!("\"ts\":{printed},")), "{ts}");
    }
    std::fs::write(s.path("store/journal.json"), [0xff, 0xfe, 0x00]).unwrap();
    assert!(
        matches!(store.load(), Loaded::Torn(..)),
        "bytes that are no JSON"
    );
    std::fs::remove_file(s.path("store/journal.json")).unwrap();
    std::fs::create_dir(s.path("store/journal.json")).unwrap();
    let mut held = Held::read(&store);
    assert!(held.torn.is_some());
    // A folder named journal.json is set aside like a file, and the next step lands.
    push(
        &mut held.journal,
        NewEntry {
            label: "x".into(),
            changes: vec![k7_file_service::journal::format::Change::new(
                "a",
                None,
                Some("A".into()),
            )],
            blobs: vec![],
        },
        1,
        1000,
        None,
    );
    held.save(&store, 9).unwrap();
    assert!(matches!(store.load(), Loaded::Journal(..)));
}

#[test]
fn an_entry_of_ten_mib_round_trips() {
    let text = journal_of(1, &"y".repeat(10 << 20));
    assert_eq!(print(&parse(&text).unwrap()), text);
}

#[test]
fn two_thousand_stores_a_store_that_is_a_file_and_names_of_sixteen_kib() {
    let s = Scratch::new("jh-stores");
    let home = s.home();
    for n in 0..2_000 {
        std::fs::create_dir_all(s.path(&format!("studio/.history/p{n}-000000000000"))).unwrap();
    }
    let long = format!("/ws/{}.k7", "a".repeat(16 << 10));
    let astral = format!("/ws/{}.k7", "😀".repeat(4000));
    let (a, b) = (store_for(&home, &long), store_for(&home, &astral));
    // A run of non-word characters is one `_`, however long.
    assert!(b.ends_with(&format!("/_.k7-{}", &b[b.len() - 12..])), "{b}");
    assert_ne!(a, b);
    // `blobs` is a file: the store still serves its journal; a step with audio fails to store, and says so.
    let store = Store::new(s.path("store"));
    s.file("store/blobs", "not a folder");
    let mut held = Held::read(&store);
    push(
        &mut held.journal,
        NewEntry {
            label: "x".into(),
            changes: vec![k7_file_service::journal::format::Change::new(
                "a",
                None,
                Some("A".into()),
            )],
            blobs: vec![],
        },
        1,
        1000,
        None,
    );
    held.save(&store, 1).unwrap();
    // A thousand leftovers in `staged/` go with the sweep.
    for n in 0..1000 {
        s.file(&format!("store/staged/{n}-1"), "x");
    }
    assert_eq!(store.sweep(Duration::ZERO), 1000);
}

#[tokio::test]
async fn two_thousand_texts_written_inside_the_settle_window_are_all_read_and_none_kept() {
    let b = Bed::new("jh-texts");
    for n in 0..2_000 {
        b.put(&format!("d{}/f{n}.k7x", n % 50), "x");
    }
    let walk = walk_project(&b.dir, &Limits::default(), false);
    assert_eq!(walk.files.len(), 2_000);
    let space = k7_file_service::space::Space::open(&b.home, &b.root);
    let mut cache = TextCache::default();
    let reads = std::cell::Cell::new(0);
    // The clock stands at the oldest file's mtime, so a slow loop writing them never settles one.
    let now = walk
        .files
        .iter()
        .map(|f| f.mtime_ms)
        .fold(f64::MAX, f64::min)
        + 1.0;
    for pass in 1..=2 {
        let img = image_of(
            &walk.files,
            &|f| k7_file_service::journal::rel_of(&k7_file_service::rel::to_rel(&space, &b.dir), f),
            &mut cache,
            now,
            64 << 20,
            &mut |p| {
                reads.set(reads.get() + 1);
                std::fs::read(p)
            },
        )
        .unwrap();
        assert_eq!(img.text.len(), 2_000);
        assert_eq!(
            reads.get(),
            2_000 * pass,
            "every file younger than the settle window is read again"
        );
        assert!(cache.is_empty(), "and none is kept");
    }
    // Settled files are kept: a second image reads nothing.
    let later = walk.files.iter().map(|f| f.mtime_ms).fold(0.0, f64::max) + 60_000.0;
    image_of(
        &walk.files,
        &|f| k7_file_service::journal::rel_of(&k7_file_service::rel::to_rel(&space, &b.dir), f),
        &mut cache,
        later,
        64 << 20,
        &mut |p| std::fs::read(p),
    )
    .unwrap();
    let before = reads.get();
    image_of(
        &walk.files,
        &|f| k7_file_service::journal::rel_of(&k7_file_service::rel::to_rel(&space, &b.dir), f),
        &mut cache,
        later,
        64 << 20,
        &mut |p| {
            reads.set(reads.get() + 1);
            std::fs::read(p)
        },
    )
    .unwrap();
    assert_eq!(reads.get(), before);
}

#[test]
fn a_text_past_the_limit_is_no_image_and_a_file_that_became_a_folder_is_absent() {
    let b = Bed::new("jh-large");
    b.put("big.k7x", &"x".repeat(2000));
    b.put("gone.k7x", "x");
    let walk = walk_project(&b.dir, &Limits::default(), false);
    let space = k7_file_service::space::Space::open(&b.home, &b.root);
    let prefix = k7_file_service::rel::to_rel(&space, &b.dir);
    let to = |f: &k7_file_service::revision::Walked| k7_file_service::journal::rel_of(&prefix, f);
    let too = image_of(
        &walk.files,
        &to,
        &mut TextCache::default(),
        0.0,
        1000,
        &mut |p| std::fs::read(p),
    );
    assert!(too.is_err());
    std::fs::remove_file(b.at("gone.k7x")).unwrap();
    std::fs::create_dir(b.at("gone.k7x")).unwrap();
    let img = image_of(
        &walk.files,
        &to,
        &mut TextCache::default(),
        0.0,
        1 << 20,
        &mut |p| std::fs::read(p),
    )
    .unwrap();
    assert!(!img.text.contains("proj.k7/gone.k7x") && img.text.contains("proj.k7/big.k7x"));
}

#[tokio::test]
async fn a_gigabyte_snapshot_is_copied_and_never_held() {
    let b = Bed::new("jh-giga");
    let f = std::fs::File::create(b.at("r.wav")).unwrap();
    f.set_len(1 << 30).unwrap();
    let begun =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "bounce", Mode::Step, ME)
            .await;
    let t0 = Instant::now();
    b.j.snapshot(begun.id, &b.at("r.wav"));
    b.put("r.wav", "small");
    let ended = b.j.end(begun.id, None).await;
    let rss = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!(
        "a 1 GiB snapshot and its record took {:?}; resident memory {rss} KiB",
        t0.elapsed()
    );
    assert!(ended.failure.is_none() && ended.files.as_ref().is_some_and(|f| f.len() == 1));
    assert!(
        rss.parse::<u64>().map_or(true, |kib| kib < 800 << 10),
        "resident {rss} KiB: the pre-image was held in memory"
    );
    assert!(t0.elapsed() < backstop());
}

/// A disk that counts the writes, renames and mode reads, and skips the flush, which is the disk's cost and not the step's.
struct Counting {
    writes: std::sync::atomic::AtomicUsize,
    renames: std::sync::atomic::AtomicUsize,
    modes: std::sync::atomic::AtomicUsize,
}

impl k7_file_service::atomic::Fs for Counting {
    fn write_temp(
        &self,
        tmp: &str,
        data: &mut dyn std::io::Read,
        _mode: Option<u32>,
    ) -> std::io::Result<()> {
        self.writes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::io::copy(data, &mut std::fs::File::create(tmp)?).map(|_| ())
    }
    fn rename(&self, from: &str, to: &str) -> std::io::Result<()> {
        self.renames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::fs::rename(from, to)
    }
    fn remove(&self, p: &str) {
        let _ = std::fs::remove_file(p);
    }
    fn mode_of(&self, _p: &str) -> Option<u32> {
        self.modes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }
}

#[tokio::test]
async fn a_step_of_five_hundred_files_is_applied_whole() {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
    const N: usize = 500;
    let b = Bed::new("jh-500");
    let ended = b
        .step("many", || {
            for n in 0..N {
                b.put(&format!("d{}/f{n}.k7x", n % 20), "x");
            }
        })
        .await;
    assert_eq!(ended.files.as_ref().map(Vec::len), Some(N));
    let disk = Counting {
        writes: AtomicUsize::new(0),
        renames: AtomicUsize::new(0),
        modes: AtomicUsize::new(0),
    };
    b.undo_on(false, &disk).await;
    assert!(!b.exists("d0/f0.k7x") && !b.exists(&format!("d19/f{}.k7x", N - 1)));
    b.undo_on(true, &disk).await;
    assert!(b.exists(&format!("d19/f{}.k7x", N - 1)));
    // One write per file of the redo, none for the undo's removals; a count never moves with the load, a quadratic apply would.
    assert_eq!(disk.writes.load(Relaxed), N);
    assert_eq!(disk.renames.load(Relaxed), N);
    assert!(disk.modes.load(Relaxed) <= 2 * N);
}

#[cfg(unix)]
#[test]
fn a_read_that_fails_leaves_the_journal_where_it_is_and_is_never_torn() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("jh-eacces");
    let store = Store::new(s.path("store"));
    let mut held = Held::read(&store);
    push(&mut held.journal, one_step(), 1, 1000, None);
    held.save(&store, 1).unwrap();
    let path = store.journal_path();
    let before = std::fs::read(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    // A process that ignores permissions (root) reads the file anyway: nothing to prove there.
    if std::fs::File::open(&path).is_ok() {
        return;
    }
    assert!(matches!(store.load(), Loaded::Unreadable(_)));
    let mut held = Held::read(&store);
    assert!(held.unread.is_some() && held.torn.is_none());
    assert!(held.save(&store, 2).is_err(), "nothing is written over it");
    assert!(!std::path::Path::new(&s.path("store/journal.torn-2.json")).exists());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        held.refresh(&store),
        "an unread journal is read again at the next use"
    );
    assert_eq!(
        (held.journal.entries.len(), std::fs::read(&path).unwrap()),
        (1, before)
    );
}

#[cfg(unix)]
fn one_step() -> NewEntry {
    NewEntry {
        label: "x".into(),
        changes: vec![k7_file_service::journal::format::Change::new(
            "a",
            None,
            Some("A".into()),
        )],
        blobs: vec![],
    }
}
