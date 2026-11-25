// cache.rs — a compile cache kept from one line to the next: the same song as compiling from scratch, and what it saves.
// A reader would get wrong what is reused: decoded files by path and stamp, so a file a verb wrote is decoded again.

mod support;

use song::Cache;
use std::time::Instant;
use support::Project;
use verbs::{VerbRequest, run_request_cached};

const LINES: [&str; 4] = [
    "project tempo 96",
    "event move drums 1:1:1 2:1:1",
    "event gain drums 1:3:1 -2",
    "track new perc",
];

fn request(p: &Project, line: &str) -> VerbRequest {
    VerbRequest {
        line: line.to_owned(),
        project: p.dir.clone(),
        env: serde_json::json!({ "studioHome": "/home-none", "metaRoot": "/k7/meta" }),
    }
}

/// Runs every line over one project, one cache kept for all of them or a fresh one each; the seconds spent.
fn run_all(p: &mut Project, kept: bool) -> f64 {
    let mut cache = Cache::new();
    let mut n = 0;
    let t = Instant::now();
    for line in LINES {
        if !kept {
            cache = Cache::new();
        }
        let req = request(p, line);
        let a = run_request_cached(
            &mut p.fs,
            &mut cache,
            &mut || {
                n += 1;
                format!("id-{n}")
            },
            &req,
        );
        assert!(!a.refused, "`{line}`: {:?} {:?}", a.refuse, a.threw);
    }
    t.elapsed().as_secs_f64()
}

#[test]
fn a_kept_cache_compiles_the_song_a_fresh_one_does() {
    let mut kept = Project::open("clip-lane");
    let mut fresh = Project::open("clip-lane");
    run_all(&mut kept, true);
    run_all(&mut fresh, false);
    assert_eq!(kept.files(), fresh.files(), "the same lines write the same files");
    assert_eq!(kept.doc(), fresh.doc());
    assert!(kept.text("project.k7x").contains("96"));
}

// The numbers the port reports (docs/verbs-port.md): `cargo test --release --test cache -- --ignored --nocapture`.
#[test]
#[ignore]
fn what_the_cache_saves() {
    for kept in [false, true] {
        let mut total = 0.0;
        for _ in 0..10 {
            let mut p = Project::open("clip-lane");
            total += run_all(&mut p, kept);
        }
        println!("cache kept: {kept}: {:.2} ms per line", total * 1000.0 / 40.0);
    }
}
