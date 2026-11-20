//! The load machine (2220) against the TypeScript's traces: each scenario of `vectors/engine.json`'s `load` is replayed on a
//! turned clock, through a small front that does what `sendDoc` and `warmDeps` do, and must say the same frames at the same times.

use k7_file_service::engine::load::Load;
use k7_file_service::engine::{Effect, Event, Phase};
use serde_json::json;

use crate::support::engine::{Clock, rows, vectors};

#[test]
fn record_is_put_back_after_the_dry_run() {
    // The one trace that holds the mutation: record on before the dry run, off for it, on again after the stop.
    let cases = vectors()["load"].as_array().unwrap();
    let case = cases
        .iter()
        .find(|c| c["name"].as_str().unwrap().starts_with("record on before"))
        .unwrap();
    let calls = rows(&case["calls"]);
    let at = |s: &str| calls.iter().position(|(_, w)| w == s).unwrap();
    assert!(at("set-record false") < at("transport play"));
    assert!(at("transport rewind") < at("set-record true"), "{calls:?}");
}

fn run_of(fx: &[Effect]) -> u64 {
    match fx {
        [Effect::AskLoaded { run: Some(r) }] => *r,
        other => panic!("one ask naming a run, got {other:?}"),
    }
}

#[test]
fn two_runs_on_two_links_answer_each_its_own_ask() {
    // F1: after a takeover both runs ask, and a swapped answer must not reach the other's wait.
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let doc = json!({ "masterVolume": -3 });
    let first = run_of(&l.warm_up(1, doc.clone(), clock.at(0)));
    let second = run_of(&l.warm_up(2, doc, clock.at(100)));
    assert_ne!(first, second);
    let fx = l.on(
        Event::Loaded {
            run: Some(second),
            held: Some(true),
        },
        clock.at(150),
    );
    assert_eq!(
        l.phase(),
        Some(Phase::Warming),
        "the newer run's answer warms it: {fx:?}"
    );
    let late = l.on(
        Event::Loaded {
            run: Some(first),
            held: Some(true),
        },
        clock.at(160),
    );
    assert!(
        late.is_empty(),
        "the replaced run ends without a word: {late:?}"
    );
    assert_eq!(l.phase(), Some(Phase::Warming));
}

#[test]
fn a_run_asked_and_never_answered_gives_up_on_its_clock() {
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let id = run_of(&l.warm_up(1, json!({}), clock.at(0)));
    let mut asks = 1;
    let mut said = vec![];
    let mut t = 0;
    while t <= 125_000 && l.phase().is_some() {
        t += 100;
        for e in l.on(Event::Tick(clock.at(t)), clock.at(t)) {
            match e {
                Effect::AskLoaded { run } => {
                    assert_eq!(run, Some(id));
                    asks += 1;
                }
                Effect::Log(m) => said.push(m),
                Effect::Restore => {}
                other => panic!("{other:?}"),
            }
        }
    }
    assert_eq!(
        l.phase(),
        None,
        "a load whose engine never answers lets the gate go"
    );
    assert!(said[0].contains("in time"), "{said:?}");
    // One ask per 2 s wait and 300 ms pause, not one for ever.
    assert!((50..=60).contains(&asks), "{asks} asks");
}

#[test]
fn an_engine_error_frame_during_the_dry_run_changes_nothing_but_a_failed_send_ends_it() {
    // F2: an unrelated `error` frame must not restore the master while the transport rolls.
    let clock = Clock::new();
    let mut l = Load::new();
    l.begin();
    let id = run_of(&l.warm_up(1, json!({ "masterVolume": -3 }), clock.at(0)));
    l.on(
        Event::Loaded {
            run: Some(id),
            held: Some(true),
        },
        clock.at(10),
    );
    assert!(l.is_warming());
    assert!(
        l.on(Event::EngineError("unrelated".into()), clock.at(20))
            .is_empty()
    );
    assert!(l.is_warming());
    let fx = l.on(Event::SendFailed("boom".into()), clock.at(30));
    assert!(fx.contains(&Effect::Restore), "{fx:?}");
    assert!(!fx.contains(&Effect::Stop), "nothing was played: {fx:?}");
    assert_eq!(l.phase(), None);
}
