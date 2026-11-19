//! jobs_hostile.rs — the models answer hostile input without a hang: 64 KiB refs and sources, 10,000 `@`, 1,000 stems, 10,000 thaws.
//! A reader gets wrong that the bound is a hang backstop (15 s, 60 under musl, best of three), never a measure: sizes are the proof.

use crate::support::jobs::Tmp;
use k7_file_service::jobs::FreezeEvent as Ev;
use k7_file_service::jobs::freezer::Freezer;
use k7_file_service::jobs::manifest::{OUTSIDE, move_into, parse_manifest};
use k7_file_service::jobs::plugin::resolve_plugin;
use k7_file_service::jobs::separations::separation_id;
use serde_json::json;
use song::modules::Scan;
use std::time::{Duration, Instant};

const BACKSTOP: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

/// Runs `f` three times and checks the fastest stayed under the backstop.
fn within<R>(mut f: impl FnMut() -> R) -> R {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t = Instant::now();
        last = Some(f());
        best = best.min(t.elapsed());
    }
    assert!(best < BACKSTOP, "{best:?}");
    last.unwrap()
}

#[test]
fn a_ref_of_64_kib_is_one_sentence_and_one_scan_of_the_catalogue() {
    let scan = Scan::default();
    let big = format!("@engine/{}", "x".repeat(64 * 1024));
    let err = within(|| resolve_plugin(&scan, &big, "", "").unwrap_err());
    assert_eq!(err, (404, format!("no scanned plugin for \"{big}\"")));
    assert_eq!(
        resolve_plugin(&scan, "", &"f".repeat(64 * 1024), &"i".repeat(64 * 1024))
            .unwrap()
            .0
            .len(),
        64 * 1024
    );
}

#[test]
fn ids_manifests_and_moves_of_hostile_size_end_fast() {
    let outputs: Vec<String> = (0..1000).map(|i| format!("o{i}")).collect();
    let id = within(|| separation_id(&"u".repeat(64 * 1024), "/s", "/m", &outputs));
    assert_eq!(id.len(), 16);
    let big = "x".repeat(64 * 1024);
    assert_eq!(
        parse_manifest(&big).unwrap_err(),
        format!("could not read the engine's render manifest:\n{big}")
    );
    let lines = "log\n".repeat(100_000) + "{\"mix\":\"a.wav\"}";
    assert_eq!(parse_manifest(&lines).unwrap(), json!({ "mix": "a.wav" }));
    let tmp = Tmp::new("jobshostilemove");
    tmp.write("s/mix.wav", "m");
    let stems: Vec<_> = (0..1000)
        .map(|i| json!({ "name": format!("s{i}"), "file": format!("/etc/{i}/{big}.wav") }))
        .collect();
    let moved = within(|| {
        move_into(
            &json!({ "mix": tmp.at("s/mix.wav"), "stems": stems }),
            &tmp.at("s"),
            &tmp.at("dest"),
        )
    });
    assert_eq!(moved, Err(OUTSIDE.to_owned()));
    assert!(
        !std::path::Path::new(&tmp.at("dest")).exists(),
        "a refusal made no folder"
    );
}

/// The names a run of `n` thaws of one folder sends back, summed over its `SetThawed` effects.
fn thaw_cost(n: usize) -> usize {
    let now = Instant::now();
    let mut f = Freezer::new();
    let mut cost = 0;
    for i in 0..n {
        let fx = f.on(
            "/p",
            Ev::Thaw {
                module: format!("m{i}"),
                on: true,
                now,
            },
        );
        assert_eq!(fx.len(), 1);
        cost += fx
            .iter()
            .map(|e| match e {
                k7_file_service::jobs::FreezeEffect::SetThawed(l) => l.len(),
                _ => 0,
            })
            .sum::<usize>();
    }
    assert_eq!(f.thawed("/p").len(), n);
    cost
}

#[test]
fn a_thousand_thaws_of_one_folder_are_a_thousand_effects_and_their_cost_is_counted() {
    // Each thaw tells the whole list, so the names sent are n(n+1)/2: the count is the model of it (n against 4n).
    assert_eq!(thaw_cost(1_000), 1_000 * 1_001 / 2);
    assert_eq!(thaw_cost(4_000), 4_000 * 4_001 / 2);
}
