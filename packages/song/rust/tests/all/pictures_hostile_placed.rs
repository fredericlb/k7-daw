// pictures_hostile_placed.rs — hostile input to the timelines' pictures, and the capacity row measured over the whole pipeline.
// A reader would get wrong why this is a file apart: pictures_hostile.rs holds the sound's half and passes 400 lines on its own.

use crate::clips_support;
use crate::pictures_support;
use crate::support;

use clips_support::{clip_family, grid, world};
use pictures_support::{lanes_of, placements};
use serde_json::{Value, json};
use song::Severity;
use song::clips::{Materials, Placer};
use song::pictures::{
    BUDGET, ClipSounds, LaneShots, Sounder, Spend, picture_lane_sounds, timeline_shots, with_timeline_shots,
};
use std::time::{Duration, Instant};

fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(
        t.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{what} took {:?}",
        t.elapsed()
    );
    out
}

fn mats(timelines: &[(String, Value)]) -> Materials {
    let refs: Vec<(&str, Value)> = timelines.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    world(&[("a", json!({"source": "clips/a.mp4"}))], &[], &refs)
}

/// The shots of `events` placed against `m`: how many per lane, the spend, and the Placer's findings.
fn run(m: &Materials, events: &[Value]) -> (Vec<(String, usize)>, Spend) {
    let evs = placements(events);
    let mut placer = Placer::new(grid(120.0), m);
    let mut spend = Spend::new();
    spend.in_file("nodes/gtr.k7x");
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut spend, &mut out);
    (out.iter().map(|(n, v)| ((*n).to_owned(), v.len())).collect(), spend)
}

fn place(r: &str, n: usize) -> Vec<Value> {
    (0..n).map(|_| json!({"ref": r, "start": "1:1"})).collect()
}

fn one_shot() -> Value {
    json!([{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}])
}

fn errors(spend: &Spend) -> Vec<&str> {
    spend
        .findings()
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.as_str())
        .collect()
}

#[test]
fn six_levels_of_a_hundred_events_stop_at_the_budget_with_one_error() {
    // decisions 522 and 523: 10^12 visits from a few kilobytes
    let mut tl: Vec<(String, Value)> = (0..5)
        .map(|i| {
            (
                format!("t{i}"),
                json!({"length": "1:0", "lanes": [{"lane": "x", "events": place(&format!("t{}", i + 1), 100)}]}),
            )
        })
        .collect();
    tl.push((
        "t5".into(),
        json!({"length": "1:0", "lanes": [], "pictures": one_shot()}),
    ));
    let m = mats(&tl);
    let (_, spend) = fast("six levels", || run(&m, &place("t0", 1)));
    assert!(spend.capped());
    assert_eq!(errors(&spend).len(), 1);
    assert!(errors(&spend)[0].starts_with("timelines/t"));
}

#[test]
fn a_nest_of_a_hundred_thousand_timelines_is_cut_at_thirty_two() {
    // decision 518
    let tl: Vec<(String, Value)> = (0..100_000)
        .map(|i| {
            let inner = if i < 99_999 {
                place(&format!("t{}", i + 1), 1)
            } else {
                vec![]
            };
            (
                format!("t{i}"),
                json!({"length": "1:0", "lanes": [{"lane": "x", "events": inner}], "pictures": one_shot()}),
            )
        })
        .collect();
    let m = mats(&tl);
    let (lanes, spend) = fast("nest", || run(&m, &place("t0", 1)));
    assert_eq!(lanes, [("cam".to_owned(), 32)]);
    assert!(spend.findings().is_empty() && !spend.capped());
}

#[test]
fn a_timeline_in_itself_and_a_cycle_of_ten_events_each_way_are_cut_silently() {
    // decision 518
    let me = json!({"length": "1:0", "lanes": [{"lane": "x", "events": place("me", 10)}], "pictures": one_shot()});
    let a = json!({"length": "1:0", "lanes": [{"lane": "x", "events": place("b", 10)}], "pictures": one_shot()});
    let b = json!({"length": "1:0", "lanes": [{"lane": "x", "events": place("a", 10)}], "pictures": one_shot()});
    let m = mats(&[("me".into(), me), ("a".into(), a), ("b".into(), b)]);
    let (lanes, spend) = run(&m, &place("me", 1));
    assert_eq!(lanes, [("cam".to_owned(), 1)]);
    let (lanes, spend2) = run(&m, &place("a", 1));
    assert_eq!(lanes, [("cam".to_owned(), 11)]);
    assert!(spend.findings().is_empty() && spend2.findings().is_empty());
}

#[test]
fn one_lane_appended_a_hundred_thousand_times_is_linear() {
    // decision 520: `entry` and `extend`, not a copy per placement
    let m = mats(&[(
        "t".into(),
        json!({"length": "1:0", "lanes": [], "pictures": one_shot()}),
    )]);
    let (lanes, spend) = fast("append", || run(&m, &place("t", 100_000)));
    assert_eq!(lanes, [("cam".to_owned(), 100_000)]);
    assert!(!spend.capped() && spend.spent() < BUDGET / 2, "spent {}", spend.spent());
}

#[test]
fn a_timeline_of_a_hundred_thousand_shots_placed_a_thousand_times_is_stopped() {
    // decisions 522 and 523
    let events: Vec<Value> = (0..100_000)
        .map(|i| json!({"ref": "a", "start": i, "length": 1}))
        .collect();
    let big = json!({"length": "100000:0", "lanes": [], "pictures": [{"lane": "cam", "events": events}]});
    let m = mats(&[("big".into(), big)]);
    let (lanes, spend) = fast("fan-out", || run(&m, &place("big", 1_000)));
    assert!(spend.capped());
    assert_eq!(errors(&spend), ["timelines/big.k7x"]);
    assert!(lanes.iter().map(|l| l.1).sum::<usize>() < BUDGET);
}

#[test]
fn a_hundred_thousand_disabled_entries_placed_many_times_are_charged_and_stopped() {
    // decision 522: a lane or an entry visited weighs 1, disabled or not
    let off_pictures: Vec<Value> = (0..100_000)
        .map(|_| json!({"lane": "cam", "disabled": true, "events": []}))
        .collect();
    let off_lanes: Vec<Value> = (0..100_000)
        .map(|_| json!({"lane": "x", "disabled": true, "events": []}))
        .collect();
    let m = mats(&[
        (
            "p".into(),
            json!({"length": "1:0", "lanes": [], "pictures": off_pictures}),
        ),
        (
            "l".into(),
            json!({"length": "1:0", "lanes": off_lanes, "pictures": one_shot()}),
        ),
    ]);
    for name in ["p", "l"] {
        let (_, spend) = fast(name, || run(&m, &place(name, 300_000)));
        assert!(spend.capped(), "{name}");
        assert_eq!(errors(&spend), [format!("timelines/{name}.k7x")]);
    }
}

#[test]
fn a_timecode_of_a_megabyte_is_weighed_each_time_it_is_parsed() {
    // decision 554: a shot's start and an event's start are text parsed per placement
    let big = "1".repeat(1 << 20);
    let shot = json!({"length": "1:0", "lanes": [], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": big, "length": 1}]}]});
    let inner = json!({"length": "1:0", "lanes": [], "pictures": one_shot()});
    let outer =
        json!({"length": "1:0", "lanes": [{"lane": "x", "events": [{"ref": "u", "start": big}]}], "pictures": []});
    let m = mats(&[("s".into(), shot), ("u".into(), inner), ("o".into(), outer)]);
    for name in ["s", "o"] {
        let (_, spend) = fast(name, || run(&m, &place(name, 20_000)));
        assert!(spend.capped(), "{name}");
        assert_eq!(errors(&spend), [format!("timelines/{name}.k7x")]);
    }
}

#[test]
fn a_name_of_one_megabyte_is_weighed_where_it_is_looked_up() {
    // decision 522: a timeline named 1 MB places once; a lane named 1 MB placed 100,000 times is stopped
    let long = "n".repeat(1 << 20);
    let m = mats(&[(
        long.clone(),
        json!({"length": "1:0", "lanes": [], "pictures": one_shot()}),
    )]);
    let (lanes, spend) = fast("long timeline", || run(&m, &place(&long, 1)));
    assert_eq!((lanes, spend.capped()), (vec![("cam".to_owned(), 1)], false));
    let named = json!({"length": "1:0", "lanes": [], "pictures": [{"lane": long, "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let m = mats(&[("t".into(), named)]);
    let (_, spend) = fast("long lane", || run(&m, &place("t", 100_000)));
    assert!(spend.capped());
    assert_eq!(errors(&spend), ["timelines/t.k7x"]);
}

#[test]
fn eight_lanes_of_two_thousand_shots_and_a_timeline_placed_thirty_two_times_fit_in_a_third_of_the_budget() {
    // the capacity row: 123 bpm, 120-character paths, half the shots overlapping, 32,000 shots in all
    let word = |k: usize| format!("clips/{}{k}.wav", "s".repeat(105));
    let clips = clip_family(
        &(0..50)
            .map(|k| {
                (
                    format!("c{k}"),
                    json!({"source": format!("clips/c{k}.mp4"), "sound": word(k)}),
                )
            })
            .collect::<Vec<_>>()
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect::<Vec<_>>(),
    );
    let wavs: Vec<String> = (0..50).map(word).collect();
    let held: Vec<(&str, &str)> = wavs.iter().map(|w| (w.as_str(), "")).collect();
    let src = support::mem(&held);
    let big_events: Vec<Value> = (0..500)
        .map(|j| json!({"ref": format!("c{}", j % 50), "start": j, "length": 1}))
        .collect();
    let big = json!({"length": "270:0", "lanes": [], "pictures": [{"lane": "cam0", "events": big_events}]});
    let m = world(&[("a", json!({"source": "clips/a.mp4"}))], &[], &[("big", big)]);
    let sound_events: Vec<Value> = (0..32)
        .map(|k| json!({"ref": "big", "start": format!("{}:1", 1 + 270 * k)}))
        .collect();
    let sound_events = placements(&sound_events);
    let lanes: Vec<(String, Vec<Value>)> = (0..8)
        .map(|l| {
            let evs = (0..2_000)
                .map(|j| json!({"ref": format!("c{}", (j * 7 + l) % 50), "start": j, "length": if j % 2 == 0 { 1 } else { 2 }}))
                .collect();
            (format!("cam{l}"), evs)
        })
        .collect();
    let owned = lanes_of(&lanes.iter().map(|(n, e)| (n.as_str(), e.clone())).collect::<Vec<_>>());
    let t = Instant::now();
    let mut placer = Placer::new(grid(123.0), &m);
    let mut spend = Spend::new();
    spend.in_file("nodes/gtr.k7x");
    let mut placed = LaneShots::new();
    timeline_shots(&sound_events, &mut placer, &mut spend, &mut placed);
    let merged = with_timeline_shots(
        owned.iter().map(|(n, e)| (n.as_str(), e.as_slice())),
        placed,
        &mut spend,
        &|n| format!("nodes/{n}.k7x"),
    );
    let total: usize = merged.values().map(Vec::len).sum();
    let sounds = ClipSounds::of(&clips);
    let out = {
        let mut s = Sounder::new(&src, "/p", 123.0 / 60.0, &sounds, &mut spend);
        picture_lane_sounds(&mut s, &[], &merged, &|_| false, &|id| format!("nodes/{id}.k7x")).expect("own")
    };
    let pieces: usize = out.values().map(Vec::len).sum();
    eprintln!(
        "capacity: {total} shots, {pieces} pieces, spent {} of {BUDGET} in {:?}",
        spend.spent(),
        t.elapsed()
    );
    assert!(t.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
    assert_eq!(total, 32_000);
    assert!(pieces > 16_000 && !spend.capped());
    assert!(spend.spent() <= BUDGET / 3, "spent {}", spend.spent());
    assert!(spend.findings().is_empty(), "{:?}", spend.findings());
}
