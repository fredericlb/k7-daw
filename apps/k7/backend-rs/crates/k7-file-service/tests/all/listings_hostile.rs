//! Hostile listings: wide and deep folders, large and many playlists, a link loop, ten thousand sounds, two
//! thousand versions. Each answered fast, asserting the answer and the time on the best of three, against a bound a busy machine keeps.

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::libraries::library_of_calls;
use k7_file_service::listings::{Listed, Listing, Remote, list};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::Value;
use support::{Scratch, can_link, link};

// Generous: a loaded machine runs an unoptimized build several times slower, and only a quadratic walk misses it.
const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn run(s: &Scratch, which: Listing, limits: &Limits) -> Result<Listed, Declined> {
    let h = s.home(false);
    s.dir("ws");
    list(
        &Space::open(&h, &s.path("ws")),
        which,
        &Remote::default(),
        limits,
    )
}

/// `n` empty files `<dir>/<prefix><i><suffix>`, written by eight threads: a folder this wide fills slowly on one.
fn fill(dir: &str, n: u32, prefix: &str, suffix: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::thread::scope(|scope| {
        for part in 0..8u32 {
            scope.spawn(move || {
                for i in (part * n / 8)..((part + 1) * n / 8) {
                    std::fs::File::create(format!("{dir}/{prefix}{i:06}{suffix}")).unwrap();
                }
            });
        }
    });
}

/// The lookups of `library_of` three runs made on this thread: a lookup per entry is the superlinear walk a count catches.
fn lookups<T>(f: impl FnOnce() -> T) -> (u64, T) {
    let before = library_of_calls();
    let out = f();
    (library_of_calls() - before, out)
}

/// The best of three runs, so a busy machine's one slow turn is not the code's.
fn best<T>(mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        last = Some(f());
        best = best.min(t0.elapsed());
    }
    (best, last.unwrap())
}

fn count(l: &Listed, key: &str) -> usize {
    serde_json::from_str::<Value>(&l.json).unwrap()[key]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn twenty_thousand_references_are_listed_fast_and_past_the_limit_declined_faster() {
    many_references(20_000);
}

#[test]
#[ignore = "scale: a quadratic walk hides at 20,000 references and shows at 200,000"]
fn two_hundred_thousand_references_are_listed_fast_and_past_the_limit_declined_faster_at_scale() {
    many_references(200_000);
}

fn many_references(n: u32) {
    let s = Scratch::new("h-refs");
    fill(&s.dir("ws/_references"), n, "t", ".wav");
    let (asked, (took, l)) =
        lookups(|| best(|| run(&s, Listing::References, &Limits::default()).unwrap()));
    assert_eq!(count(&l, "references"), n as usize);
    // The lookups prove the walk; the time only catches a quadratic, which a wide folder makes slow.
    assert!(took < BOUND * 4, "{took:?}");
    assert!(asked < 30, "{asked} lookups");
    let few = Limits {
        max_seen: u64::from(n) / 2,
        ..Limits::default()
    };
    let t0 = Instant::now();
    assert_eq!(
        run(&s, Listing::References, &few).unwrap_err(),
        Declined::TooMany
    );
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_nest_three_hundred_deep_is_declined_and_the_stack_holds() {
    let s = Scratch::new("h-deep");
    s.dir(&format!("ws/_references/{}", "d/".repeat(300)));
    s.dir(&format!("libraries/k7-core/chains/{}", "d/".repeat(300)));
    let t0 = Instant::now();
    assert_eq!(
        run(&s, Listing::References, &Limits::default()).unwrap_err(),
        Declined::TooDeep
    );
    assert_eq!(
        run(&s, Listing::LibraryChains, &Limits::default()).unwrap_err(),
        Declined::TooDeep
    );
    // Within the limit it finishes, on a thread with a small stack: nothing recurses.
    let (home, ws) = (s.home(false), s.path("ws"));
    let limits = Limits {
        max_depth: 400,
        ..Limits::default()
    };
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let space = Space::open(&home, &ws);
            for which in [Listing::References, Listing::LibraryChains] {
                assert!(list(&space, which, &Remote::default(), &limits).is_ok());
            }
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_link_loop_below_the_folder_is_never_followed() {
    let s = Scratch::new("h-loop");
    if !can_link(&s) {
        return;
    }
    s.file("ws/_references/a.wav", "");
    link(&s.path("ws/_references"), &s.path("ws/_references/loop")).unwrap();
    let l = run(&s, Listing::References, &Limits::default()).unwrap();
    assert_eq!(count(&l, "references"), 1);
}

#[test]
fn two_thousand_playlists_and_a_text_of_a_hundred_thousand_items() {
    let s = Scratch::new("h-playlists");
    let dir = s.dir("ws/_playlists");
    std::thread::scope(|scope| {
        for part in 0..8u32 {
            let dir = &dir;
            scope.spawn(move || {
                for i in (part * 250)..((part + 1) * 250) {
                    std::fs::write(
                        format!("{dir}/p{i:05}.k7x"),
                        "title(\"T\")\nitems([\"a\", \"b\"])\n",
                    )
                    .unwrap();
                }
            });
        }
    });
    let items: Vec<String> = (0..100_000).map(|i| format!("\"song {i}\"")).collect();
    s.file(
        "ws/_playlists/huge.k7x",
        &format!("items([{}])\n", items.join(", ")),
    );
    let (took, l) = best(|| run(&s, Listing::Playlists, &Limits::default()).unwrap());
    assert_eq!(count(&l, "playlists"), 2_001);
    assert!(l.json.contains(r#""name":"huge","count":100000,"ok":true"#));
    assert!(took < BOUND, "{took:?}");
}

#[test]
fn a_sixteen_mib_playlist_reads_and_one_past_the_limit_is_declined_before_it_is_read() {
    let s = Scratch::new("h-big");
    let line = "\"a song of some length\", ";
    let body = line.repeat((16 << 20) / line.len());
    s.file(
        "ws/_playlists/sixteen.k7x",
        &format!("items([{body}\"z\"])\n"),
    );
    let (took, l) = best(|| run(&s, Listing::Playlists, &Limits::default()).unwrap());
    assert!(
        l.json.contains(r#""ok":true"#),
        "{}",
        &l.json[..200.min(l.json.len())]
    );
    assert!(took < BOUND, "{took:?}");
    let max = Limits::default().max_text;
    let f = std::fs::File::create(s.path("ws/_playlists/over.k7x")).unwrap();
    f.set_len(max + 1).unwrap();
    let t0 = Instant::now();
    assert_eq!(
        run(&s, Listing::Playlists, &Limits::default()).unwrap_err(),
        Declined::TooLarge
    );
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn two_thousand_chains_and_two_hundred_libraries() {
    let s = Scratch::new("h-families");
    fill(&s.dir("libraries/k7-core/chains"), 2_000, "c", ".k7x");
    for i in 0..200 {
        s.file(&format!("libraries/lib-{i:03}/chains/one.k7x"), "");
    }
    let (asked, (took, l)) =
        lookups(|| best(|| run(&s, Listing::LibraryChains, &Limits::default()).unwrap()));
    assert_eq!(count(&l, "chains"), 2_200);
    assert!(took < BOUND, "{took:?}");
    // A few lookups a library for its root and its spelling, never one a chain.
    assert!(asked < 3 * 201 * 8, "{asked} lookups");
}

#[test]
fn a_sound_family_of_ten_thousand_samples_is_a_level_listing_fast() {
    sound_family(10_000);
}

#[test]
#[ignore = "scale: a quadratic level listing hides at 10,000 samples and shows at 100,000"]
fn a_sound_family_of_a_hundred_thousand_samples_is_a_level_listing_fast_at_scale() {
    sound_family(100_000);
}

fn sound_family(n: u32) {
    let s = Scratch::new("h-sounds");
    s.file("libraries/k7-core/modules/audio/eq/module.k7x", "");
    fill(&s.dir("libraries/k7-core/samples"), n, "s", ".wav");
    let (asked, (took, l)) =
        lookups(|| best(|| run(&s, Listing::ModuleLibraries, &Limits::default()).unwrap()));
    assert!(l.json.contains("\"samples\":[{\"name\":\"s000000.wav\""));
    assert!(took < BOUND, "{took:?}");
    assert!(asked < 100, "{asked} lookups");
}

#[test]
fn an_exts_json_of_eight_mib_and_one_that_is_not_json_still_answer() {
    let s = Scratch::new("h-exts");
    // Past the song crate's cap of 20,000 plugins a scan reads as "cannot say"; 16,000 padded ones are 8 MiB.
    let pad = "m".repeat(500);
    let plugins: Vec<String> = (0..16_000)
        .map(|i| format!(r#"{{"name":"Plugin number {i} of many","format":"VST3","role":"audioEffect","manufacturer":"{pad}"}}"#))
        .collect();
    let text = format!(r#"{{"entries":[{{"plugins":[{}]}}]}}"#, plugins.join(","));
    assert!(text.len() > 8 << 20);
    s.file("studio/exts.json", &text);
    let (took, l) = best(|| run(&s, Listing::ModuleLibraries, &Limits::default()).unwrap());
    assert!(l.json.contains("plugin-number-15999-of-many"));
    assert!(took < BOUND, "{took:?}");
    s.file("studio/exts.json", "not json");
    assert!(run(&s, Listing::ModuleLibraries, &Limits::default()).is_ok());
}

#[test]
fn two_thousand_version_folders_are_read_for_the_newest() {
    let s = Scratch::new("h-versions");
    for v in 1..=2_000 {
        s.dir(&format!("libraries/k7-core/{v}"));
    }
    s.file(
        "libraries/k7-core/2000/library.k7x",
        "name(\"x\")\nversion(2000)\nrequires(1)\n",
    );
    s.file("libraries/k7-core/1/modules/audio/a/module.k7x", "");
    let (took, l) = best(|| run(&s, Listing::ModuleLibraries, &Limits::default()).unwrap());
    assert!(
        l.json.contains(r#""version":{"pinned":1,"upgrade":2000}"#),
        "{}",
        &l.json[..300.min(l.json.len())]
    );
    assert!(took < BOUND, "{took:?}");
}
