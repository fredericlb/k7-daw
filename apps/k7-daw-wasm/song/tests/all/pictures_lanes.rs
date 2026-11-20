// pictures_lanes.rs — the lanes that sound a picture lane: `video_source:` lanes, a picture lane's own half, and their faults.
// A reader would get wrong that a fault is data and an ERROR on a file together: the TypeScript threw once, after all the lanes.

use crate::clips_support;
use crate::pictures_support;
use crate::support;

use clips_support::clip_family;
use pictures_support::{lane_shots, lanes_of};
use serde_json::{Value, json};
use song::pictures::{ClipSounds, MAX_FAULTS, SoundLaneIn, Sounder, Spend, picture_lane_sounds, video_sound_lanes};
use song::{Clip, FamilyRead, Severity};

fn clips() -> FamilyRead<Clip> {
    clip_family(&[
        ("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"})),
        ("b", json!({"source": "clips/b.mp4", "sound": "clips/b.wav"})),
        ("mute", json!({"source": "clips/mute.mp4"})),
    ])
}

fn lane<'a>(name: &'a str, events: usize, from: Option<&'a str>) -> SoundLaneIn<'a> {
    SoundLaneIn {
        name,
        events,
        video_source: from,
        file: format!("nodes/{name}.k7x"),
    }
}

fn main_events() -> Vec<Value> {
    vec![
        json!({"ref": "a", "start": 0, "length": 5}),
        json!({"ref": "b", "start": 4, "length": 5, "fadeIn": 1}),
    ]
}

struct Ran {
    video: indexmap::IndexMap<String, Vec<song::clips::ClipDoc>>,
    own: indexmap::IndexMap<String, Vec<song::clips::ClipDoc>>,
    faults: Vec<String>,
    errors: Vec<String>,
}

/// Both walks over `lanes` and the picture lanes `pictures`, over a folder holding `files`.
fn run(files: &[&str], lanes: &[SoundLaneIn<'_>], pictures: &[(&str, Vec<Value>)], wired: &[&str]) -> Ran {
    let held: Vec<(&str, &str)> = files.iter().map(|f| (*f, "")).collect();
    let src = support::mem(&held);
    let c = clips();
    let sounds = ClipSounds::of(&c);
    let mut spend = Spend::new();
    let owned = lanes_of(pictures);
    let shots = lane_shots(&owned);
    let (video, own) = {
        let mut s = Sounder::new(&src, "/p", 2.0, &sounds, &mut spend);
        let video = video_sound_lanes(&mut s, lanes, &shots).expect("video");
        let own = picture_lane_sounds(&mut s, lanes, &shots, &|id| wired.contains(&id), &|id| {
            format!("nodes/{id}.k7x")
        })
        .expect("own");
        (video, own)
    };
    let (findings, faults) = spend.into_parts();
    let of = |sev: Severity| {
        findings
            .iter()
            .filter(|f| f.severity == sev)
            .map(|f| f.file.clone())
            .collect()
    };
    Ran {
        video,
        own,
        faults: faults.iter().map(song::pictures::SoundFault::key).collect(),
        errors: of(Severity::Error),
    }
}

const BOTH: &[&str] = &["clips/a.wav", "clips/b.wav"];

#[test]
fn a_lane_naming_a_picture_lane_plays_its_shots_sound_files() {
    // videoSound.test.ts "gives the lane the shots' sound files", decision 516
    let r = run(
        BOTH,
        &[lane("dialog", 0, Some("main"))],
        &[("main", main_events())],
        &[],
    );
    let got: Vec<(&str, f64)> = r.video["dialog"]
        .iter()
        .map(|c| (c.source.as_str(), c.fade_out + c.fade_in))
        .collect();
    assert_eq!(got, [("/p/clips/a.wav", 2.0), ("/p/clips/b.wav", 2.0)]);
    assert!(r.faults.is_empty() && r.errors.is_empty());
}

#[test]
fn a_picture_lane_that_is_not_there_and_events_beside_the_link_are_faults() {
    // videoSound.test.ts "refuses a picture lane that is not there, and events beside the link", decision 516
    let r = run(
        BOTH,
        &[lane("dialog", 0, Some("nope"))],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.faults, ["video_source_absent|lane:dialog|nope"]);
    assert_eq!(r.errors, ["nodes/dialog.k7x"]);
    assert!(!r.video.contains_key("dialog"));
    let r = run(
        BOTH,
        &[lane("dialog", 1, Some("main"))],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.faults, ["video_source_both|lane:dialog|main"]);
    assert!(r.video.is_empty());
    assert!(
        !r.own.contains_key("main"),
        "a lane that is sounded has no half of its own"
    );
}

#[test]
fn a_sound_that_is_not_on_disk_is_a_fault_per_lane_and_ref_and_an_error_on_the_clip_file() {
    // videoSound.test.ts "refuses a sound file that is gone", decision 513
    let r = run(
        &["clips/a.wav"],
        &[lane("dialog", 0, Some("main"))],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.faults, ["sound_missing|lane:dialog|b"]);
    assert_eq!(r.errors, ["clips/b.k7x"]);
    assert_eq!(
        r.video["dialog"].len(),
        2,
        "the clip keeps its path; the compile stops on the error"
    );
}

#[test]
fn one_sound_on_two_shots_is_one_fault_and_a_covered_absent_sound_is_none() {
    // decision 513: distinct refs per lane, and a sound never looked for when fully covered
    let twice = vec![
        json!({"ref": "b", "start": 0, "length": 1}),
        json!({"ref": "b", "start": 2, "length": 1}),
    ];
    let r = run(&["clips/a.wav"], &[lane("x", 0, Some("main"))], &[("main", twice)], &[]);
    assert_eq!(r.faults, ["sound_missing|lane:x|b"]);
    let covered = vec![
        json!({"ref": "b", "start": 0, "length": 1}),
        json!({"ref": "a", "start": 0, "length": 1}),
    ];
    let r = run(
        &["clips/a.wav"],
        &[lane("x", 0, Some("main"))],
        &[("main", covered)],
        &[],
    );
    assert!(r.faults.is_empty() && r.errors.is_empty());
}

#[test]
fn two_lanes_and_one_absent_sound_are_two_faults() {
    // decision 513: the fault is per (lane, ref)
    let r = run(
        &["clips/a.wav"],
        &[lane("x", 0, Some("main")), lane("y", 0, Some("main"))],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.faults, ["sound_missing|lane:x|b", "sound_missing|lane:y|b"]);
    assert_eq!(
        r.errors,
        ["clips/b.k7x", "clips/b.k7x"],
        "each lane names itself in its message"
    );
}

#[test]
fn a_lane_with_no_video_source_is_skipped_and_two_lanes_may_name_one_picture_lane() {
    // decision 516 and the lanes test: both get the clips
    let r = run(
        BOTH,
        &[
            lane("plain", 0, None),
            lane("x", 0, Some("main")),
            lane("y", 0, Some("main")),
        ],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.video.keys().map(String::as_str).collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(r.video["x"].len(), 2);
}

#[test]
fn a_name_spelling_an_object_prototype_key_is_a_plain_name() {
    // decision 516 (DEPARTS, 357): `videoLanes["constructor"]` is truthy in JavaScript
    let r = run(
        BOTH,
        &[lane("x", 0, Some("constructor"))],
        &[("main", main_events())],
        &[],
    );
    assert_eq!(r.faults, ["video_source_absent|lane:x|constructor"]);
    let r = run(
        BOTH,
        &[lane("x", 0, Some("constructor"))],
        &[("constructor", main_events())],
        &[],
    );
    assert_eq!(r.video["x"].len(), 2);
}

#[test]
fn a_picture_lane_sounds_by_its_own_half_when_not_sounded_and_it_has_clips_or_is_wired() {
    // decision 517
    let silent = vec![json!({"ref": "mute", "start": 0, "length": 1})];
    let r = run(
        BOTH,
        &[lane("x", 0, Some("taken"))],
        &[
            ("main", main_events()),
            ("taken", main_events()),
            ("quiet", silent.clone()),
            ("wired", silent.clone()),
        ],
        &["wired"],
    );
    assert_eq!(r.own.keys().map(String::as_str).collect::<Vec<_>>(), ["main", "wired"]);
    assert!(r.own["wired"].is_empty() && r.own["main"].len() == 2);
    assert!(r.video["x"].len() == 2, "the lane plays it, its own half does not");
}

#[test]
fn a_picture_lane_with_no_shot_at_all_is_a_half_only_when_wired() {
    // decision 517: a wired lane with no clips
    let r = run(BOTH, &[], &[("empty", vec![]), ("wired", vec![])], &["wired"]);
    assert_eq!(r.own.keys().map(String::as_str).collect::<Vec<_>>(), ["wired"]);
}

#[test]
fn a_missing_sound_of_a_picture_lane_is_a_fault_under_picture() {
    // decision 515: the holder is `picture:<lane>`
    let r = run(&["clips/a.wav"], &[], &[("main", main_events())], &[]);
    assert_eq!(r.faults, ["sound_missing|picture:main|b"]);
    assert_eq!(r.errors, ["clips/b.k7x"]);
}

#[test]
fn a_word_the_source_refuses_costs_that_sound_and_warns_on_the_clip_file() {
    // decision 514
    let c = clip_family(&[("up", json!({"source": "a.mp4", "sound": "../../x.wav"}))]);
    let src = support::mem(&[]);
    let sounds = ClipSounds::of(&c);
    let mut spend = Spend::new();
    let owned = lanes_of(&[("main", vec![json!({"ref": "up", "start": 0, "length": 1})])]);
    let shots = lane_shots(&owned);
    let out = {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        picture_lane_sounds(&mut s, &[], &shots, &|_| true, &|id| format!("nodes/{id}.k7x")).expect("own")
    };
    assert!(out["main"].is_empty());
    assert_eq!(spend.findings().len(), 1);
    assert_eq!(spend.findings()[0].severity, Severity::Warning);
    assert_eq!(spend.findings()[0].file, "clips/up.k7x");
}

#[test]
fn past_the_faults_limit_one_more_stops_the_walk_and_each_string_stays_short() {
    // decision 515: a thousand faults and `More`, strings cut to 200 characters, messages under 300 bytes
    let long = "x".repeat(5_000);
    let names: Vec<String> = (0..1_200).map(|i| format!("{long}{i}")).collect();
    let lanes: Vec<SoundLaneIn<'_>> = names.iter().map(|n| lane(n, 0, Some(&long))).collect();
    let src = support::mem(&[]);
    let c = clips();
    let sounds = ClipSounds::of(&c);
    let mut spend = Spend::new();
    let shots = lane_shots(&[]);
    {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        assert!(video_sound_lanes(&mut s, &lanes, &shots).expect("lanes").is_empty());
    }
    assert!(spend.stopped());
    let (findings, faults) = spend.into_parts();
    assert_eq!(faults.len(), MAX_FAULTS + 1);
    assert_eq!(faults.last().map(|f| f.key()), Some("more||".to_owned()));
    assert!(
        faults
            .iter()
            .all(|f| f.holder.chars().count() <= 200 && f.target.chars().count() <= 200)
    );
    assert!(findings.iter().all(|f| f.message.len() <= 300));
    assert_eq!(findings.len(), MAX_FAULTS + 1);
}
