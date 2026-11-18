// pictures_placed.rs — the shots a placed timeline puts in the picture, and the merge that puts them before a lane's own.
// A reader would get wrong that a lane is found by NAME: the timeline is placed on a sound lane, and draws on the picture lane `cam`.

use crate::clips_support;
use crate::pictures_support;
use crate::support;

use clips_support::{grid, world};
use pictures_support::{lanes_of, placements};
use serde_json::{Value, json};
use song::clips::{Materials, Placer};
use song::pictures::{LaneShots, Spend, timeline_shots, with_timeline_shots};

type Shots = Vec<(String, Vec<(f64, f64, Option<String>)>)>;

fn mats(timelines: &[(&str, Value)]) -> Materials {
    world(&[("a", json!({"source": "clips/a.mp4"}))], &[], timelines)
}

fn shape(lanes: &LaneShots<'_>) -> Shots {
    lanes
        .iter()
        .map(|(n, v)| {
            (
                (*n).to_owned(),
                v.iter()
                    .map(|s| (s.start, s.length, s.timeline.map(str::to_owned)))
                    .collect(),
            )
        })
        .collect()
}

/// What `events` place at `tempo`, and the spend that placed it.
fn placed(m: &Materials, tempo: f64, events: &[Value]) -> (Shots, Spend) {
    let evs = placements(events);
    let mut placer = Placer::new(grid(tempo), m);
    let mut spend = Spend::new();
    spend.in_file("nodes/gtr.k7x");
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut spend, &mut out);
    (shape(&out), spend)
}

fn verse() -> Value {
    json!({
        "length": "2:0",
        "lanes": [{"lane": "x", "events": []}],
        "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}, {"ref": "a", "start": 1.5}]}]
    })
}

fn row(start: f64, length: f64, timeline: Option<&str>) -> (f64, f64, Option<String>) {
    (start, length, timeline.map(str::to_owned))
}

#[test]
fn a_timeline_draws_on_the_lane_of_its_name_before_the_lanes_own_shots() {
    // timelinePictures.test.ts "draws on the song's lane of its name … BEFORE the lane's own shots", decisions 518 to 521
    let m = mats(&[("verse", verse())]);
    let evs = placements(&[json!({"ref": "verse", "start": "3:1"})]);
    let mut placer = Placer::new(grid(120.0), &m);
    let mut spend = Spend::new();
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut spend, &mut out);
    let own = placements(&[json!({"ref": "a", "start": 0, "length": 1})]);
    let merged = with_timeline_shots([("cam", own.as_slice())].into_iter(), out, &mut spend, &|n| {
        format!("nodes/{n}.k7x")
    });
    // Bar 3 at 120 bpm is 4 s; the open last shot is given the room left, 8 - 5.5.
    assert_eq!(
        shape(&merged),
        [(
            "cam".to_owned(),
            vec![
                row(4.0, 1.0, Some("verse")),
                row(5.5, 2.5, Some("verse")),
                row(0.0, 1.0, None)
            ]
        )]
    );
}

#[test]
fn a_disabled_event_shows_nothing_and_leaves_only_the_lanes_own_shot() {
    // timelinePictures.test.ts "shows nothing where its event is disabled", decision 518
    let m = mats(&[("verse", verse())]);
    let (got, _) = placed(&m, 120.0, &[json!({"ref": "verse", "start": "3:1", "disabled": true})]);
    assert!(got.is_empty());
    let own = placements(&[json!({"ref": "a", "start": 0, "length": 1})]);
    let mut spend = Spend::new();
    let merged = with_timeline_shots(
        [("cam", own.as_slice())].into_iter(),
        LaneShots::new(),
        &mut spend,
        &|n| format!("nodes/{n}.k7x"),
    );
    assert_eq!(shape(&merged), [("cam".to_owned(), vec![row(0.0, 1.0, None)])]);
}

#[test]
fn a_ref_in_no_timeline_a_disabled_lane_and_a_disabled_picture_lane_place_nothing() {
    // decision 518
    let t = json!({
        "length": "1:0",
        "lanes": [{"lane": "x", "disabled": true, "events": [{"ref": "inner", "start": "1:1"}]}],
        "pictures": [{"lane": "cam", "disabled": true, "events": [{"ref": "a", "start": 0, "length": 1}]}]
    });
    let inner = json!({"length": "1:0", "lanes": [], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let m = mats(&[("t", t), ("inner", inner)]);
    let (got, _) = placed(
        &m,
        120.0,
        &[
            json!({"ref": "t", "start": "1:1"}),
            json!({"ref": "nope", "start": "1:1"}),
        ],
    );
    assert!(got.is_empty());
}

#[test]
fn a_shot_is_cut_at_the_timelines_end_and_one_that_starts_past_it_is_dropped() {
    // decision 519: room is `end - start`, skipped at 1e-6 or less
    let t = json!({"length": "1:0", "lanes": [], "pictures": [{"lane": "cam", "events": [
        {"ref": "a", "start": 0, "length": 1}, {"ref": "a", "start": 1.5, "length": 5}, {"ref": "a", "start": 2, "length": 1}, {"ref": "a", "start": 9, "length": 1}]}]});
    let m = mats(&[("t", t)]);
    let (got, _) = placed(&m, 120.0, &[json!({"ref": "t", "start": "1:1"})]);
    // One bar is 2 s: the second shot is cut to 0.5, the third starts at the end, the fourth is past it.
    assert_eq!(
        got,
        [(
            "cam".to_owned(),
            vec![row(0.0, 1.0, Some("t")), row(1.5, 0.5, Some("t"))]
        )]
    );
}

#[test]
fn a_shot_with_no_length_takes_the_next_entrys_start_and_the_last_takes_the_room_left() {
    // decision 519
    let t = json!({"length": "2:0", "lanes": [], "pictures": [{"lane": "cam", "events": [
        {"ref": "a", "start": 0}, {"ref": "a", "start": 1}, {"ref": "a", "start": 1}, {"ref": "a", "start": 3}]}]});
    let m = mats(&[("t", t)]);
    let (got, _) = placed(&m, 120.0, &[json!({"ref": "t", "start": "1:1"})]);
    // The second shot's next starts where it does, so it has no length and is dropped; the last runs to 4 s.
    assert_eq!(
        got,
        [(
            "cam".to_owned(),
            vec![
                row(0.0, 1.0, Some("t")),
                row(1.0, 2.0, Some("t")),
                row(3.0, 1.0, Some("t"))
            ]
        )]
    );
}

#[test]
fn a_nested_timeline_is_offset_by_the_outer_and_cut_by_the_outers_end() {
    // decision 518: `{offset: start, end}` is passed to each sound lane
    let outer = json!({"length": "1:2", "lanes": [{"lane": "x", "events": [{"ref": "inner", "start": "1:3"}]}], "pictures": []});
    let inner = json!({"length": "4:0", "lanes": [], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 9}]}]});
    let m = mats(&[("outer", outer), ("inner", inner)]);
    let (got, _) = placed(&m, 120.0, &[json!({"ref": "outer", "start": "2:1"})]);
    // outer starts at 2 s and lasts 6 beats so ends at 5 s; inner starts two beats in, at 3 s, and is cut at 5 s.
    assert_eq!(got, [("cam".to_owned(), vec![row(3.0, 2.0, Some("inner"))])]);
}

#[test]
fn a_shot_knows_whether_a_timeline_inside_another_placed_it() {
    // decision 518: the outer's own shot is direct, the inner's is nested
    let outer = json!({"length": "4:0", "lanes": [{"lane": "x", "events": [{"ref": "inner", "start": "1:1"}]}], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let inner = json!({"length": "2:0", "lanes": [], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let m = mats(&[("outer", outer), ("inner", inner)]);
    let evs = placements(&[json!({"ref": "outer", "start": "1:1"})]);
    let mut placer = Placer::new(grid(120.0), &m);
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut Spend::new(), &mut out);
    let flags: Vec<(Option<&str>, bool)> = out["cam"].iter().map(|s| (s.timeline, s.nested)).collect();
    assert_eq!(flags, [(Some("outer"), false), (Some("inner"), true)]);
}

#[test]
fn a_merge_that_runs_out_is_named_by_the_file_of_the_lane_it_stopped_at() {
    // decision 523: the file it stopped in, a later lane's included
    let small = placements(&[json!({"ref": "a", "start": 0, "length": 1})]);
    let big = placements(
        &(0..10)
            .map(|i| json!({"ref": "a", "start": i, "length": 1}))
            .collect::<Vec<_>>(),
    );
    let mut spend = Spend::with_limit(5);
    let lanes = [("cam", small.as_slice()), ("big", big.as_slice())];
    let merged = with_timeline_shots(lanes.into_iter(), LaneShots::new(), &mut spend, &|n| {
        format!("nodes/{n}.k7x")
    });
    assert_eq!(merged.len(), 1);
    let files: Vec<&str> = spend.findings().iter().map(|f| f.file.as_str()).collect();
    assert_eq!(files, ["nodes/big.k7x"]);
}

#[test]
fn a_timeline_in_itself_and_a_cycle_are_skipped() {
    // decision 518: a timeline on the path is skipped, silently
    let me = json!({"length": "1:0", "lanes": [{"lane": "x", "events": [{"ref": "me", "start": "1:1"}]}], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let a = json!({"length": "1:0", "lanes": [{"lane": "x", "events": [{"ref": "b", "start": "1:1"}]}], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let b = json!({"length": "1:0", "lanes": [{"lane": "x", "events": [{"ref": "a", "start": "1:1"}]}], "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let m = mats(&[("me", me), ("a", a), ("b", b)]);
    let (got, spend) = placed(&m, 120.0, &[json!({"ref": "me", "start": "1:1"})]);
    assert_eq!(got, [("cam".to_owned(), vec![row(0.0, 1.0, Some("me"))])]);
    assert!(spend.findings().is_empty());
    let (got, _) = placed(&m, 120.0, &[json!({"ref": "a", "start": "1:1"})]);
    assert_eq!(got[0].1.len(), 2);
}

#[test]
fn lanes_are_met_in_first_seen_order_and_one_lane_is_appended_to() {
    // decision 520: IndexMap::entry, first-seen lane order; a lane named by an integer-like key stays in file order (528)
    let t = json!({"length": "1:0", "lanes": [], "pictures": [
        {"lane": "9", "events": [{"ref": "a", "start": 0, "length": 1}]},
        {"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]},
        {"lane": "1", "events": [{"ref": "a", "start": 0, "length": 1}]}]});
    let m = mats(&[("t", t)]);
    let (got, _) = placed(
        &m,
        120.0,
        &[json!({"ref": "t", "start": "1:1"}), json!({"ref": "t", "start": "2:1"})],
    );
    assert_eq!(
        got.iter().map(|(n, v)| (n.as_str(), v.len())).collect::<Vec<_>>(),
        [("9", 2), ("cam", 2), ("1", 2)]
    );
}

#[test]
fn a_nest_past_the_depth_is_cut_at_thirty_two_without_a_word() {
    // decision 518: depth is MAX_NEST
    let items: Vec<(String, Value)> = (0..40)
        .map(|i| {
            let inner = if i < 39 {
                json!([{"ref": format!("t{}", i + 1), "start": "1:1"}])
            } else {
                json!([])
            };
            (
                format!("t{i}"),
                json!({"length": "1:0", "lanes": [{"lane": "x", "events": inner}],
                "pictures": [{"lane": "cam", "events": [{"ref": "a", "start": 0, "length": 1}]}]}),
            )
        })
        .collect();
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = mats(&refs);
    let (got, spend) = placed(&m, 120.0, &[json!({"ref": "t0", "start": "1:1"})]);
    assert_eq!(got[0].1.len(), 32);
    assert!(spend.findings().is_empty() && !spend.capped());
}

#[test]
fn a_placed_shot_sounds_through_its_audio_half_and_through_a_video_source_lane() {
    // the seam with the sound: a placed shot is a shot of the lane, heard where it is seen
    let m = mats(&[("verse", verse())]);
    let evs = placements(&[json!({"ref": "verse", "start": "3:1"})]);
    let mut placer = Placer::new(grid(120.0), &m);
    let mut spend = Spend::new();
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut spend, &mut out);
    let owned = lanes_of(&[("cam", vec![json!({"ref": "a", "start": 0, "length": 1})])]);
    let merged = with_timeline_shots(
        owned.iter().map(|(n, e)| (n.as_str(), e.as_slice())),
        out,
        &mut spend,
        &|n| format!("nodes/{n}.k7x"),
    );
    let clips = clips_support::clip_family(&[("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"}))]);
    let src = support::mem(&[("clips/a.wav", "")]);
    let sounds = song::pictures::ClipSounds::of(&clips);
    let mut s = song::pictures::Sounder::new(&src, "/p", 2.0, &sounds, &mut spend);
    let own = song::pictures::picture_lane_sounds(&mut s, &[], &merged, &|_| false, &|id| format!("nodes/{id}.k7x"))
        .expect("own");
    let starts: Vec<f64> = own["cam"].iter().map(|c| c.start).collect();
    assert_eq!(starts, [8.0, 11.0, 0.0], "the placed shots come first in the lane");
    let lane = song::pictures::SoundLaneIn {
        name: "dialog",
        events: 0,
        video_source: Some("cam"),
        file: "nodes/dialog.k7x".into(),
    };
    let video = song::pictures::video_sound_lanes(&mut s, &[lane], &merged).expect("video");
    assert_eq!(video["dialog"].len(), 3);
}

#[test]
fn a_walk_past_the_budget_is_one_error_in_the_file_it_stopped_in() {
    // decision 523
    let m = mats(&[("verse", verse())]);
    let evs = placements(&[
        json!({"ref": "verse", "start": "3:1"}),
        json!({"ref": "verse", "start": "5:1"}),
    ]);
    let mut placer = Placer::new(grid(120.0), &m);
    let mut spend = Spend::with_limit(5);
    spend.in_file("nodes/gtr.k7x");
    let mut out = LaneShots::new();
    timeline_shots(&evs, &mut placer, &mut spend, &mut out);
    assert!(spend.capped());
    assert_eq!(spend.findings().len(), 1);
    assert_eq!(spend.findings()[0].file, "timelines/verse.k7x");
}
