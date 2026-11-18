//! The capacity rows, run by hand: `/module-libraries` over eight copies of `k7-core` (1e), and 64 copies of the shape of step 0's `cap128` (2,203 files each) in one workspace,
//! the tree of the whole, its entries, bytes and time printed against the limits, each under a third of its limit.
//! `cargo test -p k7-file-service --test all -- --ignored --nocapture capacity::`

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::listings::{Listing, Remote, list};
use k7_file_service::revision::walk_sources;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::tree::{Limits, tree};
use k7_file_service::watch::Feed;
use support::Scratch;

/// One project of 128 lanes, each with 16 plugin files, 16 buses and a few files at the root: 2,203 files.
fn make(s: &Scratch, n: usize) -> usize {
    let base = format!("ws/cap{n:03}.k7");
    let mut files = 0;
    let mut put = |rel: &str| {
        s.file(&format!("{base}/{rel}"), "");
        files += 1;
    };
    for f in [
        "main.k7x",
        "graph.k7x",
        "surface.k7x",
        "notes.txt",
        "clip.wav",
        "cue.k7x",
        "tempo.k7x",
        "meta.k7x",
    ] {
        put(f);
    }
    for b in 0..16 {
        put(&format!("buses/bus-{b:02}.k7x"));
    }
    for l in 0..128 {
        put(&format!("lanes/lane-{l:03}.k7x"));
        for p in 0..16 {
            put(&format!("modules/lane-{l:03}/plugin-{p:02}.k7x"));
        }
    }
    files
}

#[test]
#[ignore = "capacity: run by hand, it writes 140,000 files"]
fn sixty_four_capacity_projects_in_one_workspace() {
    let s = Scratch::new("capacity");
    s.dir("ws");
    let per = make(&s, 0);
    for n in 1..64 {
        make(&s, n);
    }
    let h = s.home(false);
    let limits = Limits::default();
    let space = Space::open(&h, &s.path("ws"));
    let mut best = std::time::Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        last = Some(tree(&space, &limits).unwrap());
        best = best.min(t0.elapsed());
    }
    let t = last.unwrap();
    let seen_third = limits.max_seen / 3;
    let bytes_third = (limits.max_bytes / 3) as u64;
    println!("files per project: {per}");
    println!(
        "entries written: {} of the walk's {} (a third of max_seen is {seen_third})",
        t.entries, limits.max_seen
    );
    println!(
        "bytes written: {} of {} (a third of max_bytes is {bytes_third})",
        t.json.len(),
        limits.max_bytes
    );
    println!("depth: {} of {}", t.depth, limits.max_depth);
    println!("time, best of three: {best:?} (node's service, one project of this shape: 199 ms)");
    assert!(t.entries < seen_third && (t.json.len() as u64) < bytes_third);
    assert!(t.depth < limits.max_depth / 3);
}

/// A folder copied whole, links followed as plain files.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

/// A feed that is always up and never hears anything: the memo's best case.
struct Still;

impl Feed for Still {
    fn generation(&self, _root: &str) -> Option<u64> {
        Some(1)
    }
}

#[test]
#[ignore = "capacity: run by hand, it copies the repository's k7-core eight times"]
fn module_libraries_over_eight_copies_of_k7_core() {
    let s = Scratch::new("capacity-levels");
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../k7-core");
    let mut pins = String::new();
    for n in 0..8 {
        let name = if n == 0 {
            "k7-core".to_string()
        } else {
            format!("k7-core-{n}")
        };
        copy_dir(
            &repo,
            std::path::Path::new(&s.path(&format!("libraries/{name}/3"))),
        );
        pins.push_str(&format!("{name}(3)\n"));
    }
    s.file("ws/_library-pins.k7x", &pins);
    let h = s.home(false);
    let limits = Limits::default();
    let space = Space::open(&h, &s.path("ws"));
    let mut best = std::time::Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        last = Some(
            list(
                &space,
                Listing::ModuleLibraries,
                &Remote::default(),
                &limits,
            )
            .unwrap(),
        );
        best = best.min(t0.elapsed());
    }
    let l = last.unwrap();
    let seen_third = limits.max_seen / 3;
    let bytes_third = limits.max_bytes / 3;
    let entries = l.json.matches("{\"name\":").count() as u64;
    println!(
        "entries written: {entries} of the walks' {} (a third is {seen_third})",
        limits.max_seen
    );
    println!(
        "answer: {} bytes of {} (a third is {bytes_third}), {} rows",
        l.json.len(),
        limits.max_bytes,
        l.rows.len()
    );
    println!("depth: {} of {}", l.depth, limits.max_depth);
    println!(
        "time, best of three: {best:?} (node's service over eight copies: 2,359 ms of CPU, L0; over one copy without a JIT: 433 ms)"
    );
    assert!(entries < seen_third && l.json.len() < bytes_third && l.depth < limits.max_depth / 3);
}

#[test]
#[ignore = "capacity: run by hand with --release, it writes 2,200 files"]
fn one_cap128_project_walk_and_memo_hit() {
    let s = Scratch::new("capacity-revision");
    s.dir("ws");
    make(&s, 0);
    let (dir, h) = (s.path("ws/cap000.k7"), s.home(false));
    let mut walks: Vec<Duration> = (0..30)
        .map(|_| {
            let t0 = Instant::now();
            let (sum, unknowable) = walk_sources(&dir, &Limits::default());
            assert_eq!((sum.count, unknowable), (2_198, false));
            t0.elapsed()
        })
        .collect();
    walks.sort();
    println!(
        "narrow walk of 2,200 files, 132 folders: median {:?}, p90 {:?}, best {:?} (node's service: 19 to 30 ms; the order's Mac: 5.5 ms median)",
        walks[15], walks[27], walks[0]
    );
    let memo = Revisions::with_feed(Box::new(Still), Instant::now);
    let first = memo.token(&h, &s.path("ws"), Some(&dir), false);
    let mut hits: Vec<Duration> = (0..1000)
        .map(|_| {
            let t0 = Instant::now();
            assert_eq!(memo.token(&h, &s.path("ws"), Some(&dir), false), first);
            t0.elapsed()
        })
        .collect();
    hits.sort();
    println!(
        "memo hit (three stats and a lock), 1,000 asks: median {:?}, p90 {:?}; walks taken: {}",
        hits[500],
        hits[900],
        memo.walks()
    );
    assert_eq!(memo.walks(), 1);
}

/// The journal's bracket on `cap128` (2,203 files of 250 bytes), against the TypeScript's times on the page.
#[tokio::test]
#[ignore = "capacity: run by hand, it writes 2,203 files and waits for them to settle"]
async fn the_journals_bracket_on_cap128() {
    use k7_file_service::journal::Mode;
    use support::journal::{Bed, ME};
    let b = Bed::new("cap128-bracket");
    let body = "x".repeat(249) + "\n";
    let mut files = 0;
    let mut put = |rel: &str| {
        b.put(rel, &body);
        files += 1;
    };
    for f in [
        "main.k7x",
        "graph.k7x",
        "surface.k7x",
        "notes.txt",
        "cue.k7x",
        "tempo.k7x",
        "meta.k7x",
    ] {
        put(f);
    }
    for n in 0..16 {
        put(&format!("buses/bus-{n:02}.k7x"));
    }
    for l in 0..128 {
        put(&format!("lanes/lane-{l:03}.k7x"));
        for p in 0..16 {
            put(&format!("modules/lane-{l:03}/plugin-{p:02}.k7x"));
        }
    }
    println!("cap128 files: {files}");
    let bracket = async |n: usize| {
        let t0 = Instant::now();
        let begun =
            b.j.begin(&b.home, &b.root, Some(&b.dir), "edit", Mode::Step, ME)
                .await;
        b.put("main.k7x", &format!("{body}# {n}\n"));
        b.j.end_now(begun.id, None);
        t0.elapsed().as_secs_f64() * 1000.0
    };
    let young = bracket(0).await;
    println!("first bracket, every file younger than 2 s (both images read): {young:.1} ms");
    tokio::time::sleep(Duration::from_millis(2200)).await;
    let mut cached = Vec::new();
    for n in 1..=15 {
        cached.push(bracket(n).await);
    }
    cached.sort_by(f64::total_cmp);
    println!(
        "images cached (the edited file alone is young): median {:.1} ms, slowest {:.1} ms (30 ms is the brief's ceiling, 150 ms with images read)",
        cached[7], cached[14]
    );
}

#[test]
#[ignore = "capacity: profile of the bracket's parts"]
fn where_the_journals_bracket_goes() {
    use k7_file_service::journal::image::{TextCache, image_of};
    use k7_file_service::revision::walk_project;
    use support::journal::Bed;
    let b = Bed::new("cap128-parts");
    for l in 0..128 {
        for p in 0..17 {
            b.put(
                &format!("modules/lane-{l:03}/plugin-{p:02}.k7x"),
                &"x".repeat(250),
            );
        }
    }
    std::thread::sleep(Duration::from_millis(2200));
    let t = Instant::now();
    let walk = walk_project(&b.dir, &Limits::default(), false);
    println!(
        "walk_project: {:.1} ms for {} files",
        t.elapsed().as_secs_f64() * 1000.0,
        walk.files.len()
    );
    let space = Space::open(&b.home, &b.root);
    let mut cache = TextCache::default();
    let now = k7_file_service::journal::wall_ms() as f64;
    for pass in 0..2 {
        let t = Instant::now();
        image_of(
            &walk.files,
            &|f| k7_file_service::journal::rel_of(&k7_file_service::rel::to_rel(&space, &b.dir), f),
            &mut cache,
            now,
            64 << 20,
            &mut |p| std::fs::read(p),
        )
        .unwrap();
        println!(
            "image_of pass {pass}: {:.1} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    let t = Instant::now();
    for f in &walk.files {
        std::hint::black_box(k7_file_service::rel::to_rel(&space, &f.abs));
    }
    println!("to_rel alone: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
}
