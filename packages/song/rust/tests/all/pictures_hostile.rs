// pictures_hostile.rs — hostile input to every pass of the pictures, each answered fast and asserted on its answer as well as its time.
// A reader would get wrong that the row counts the OUTPUT: a 2 MB project can name 25 million pieces, and none is made past the budget.

use crate::clips_support;
use crate::pictures_support;
use crate::support;

use clips_support::clip_family;
use pictures_support::{Counting, lane_shots, own, pieces_in, pieces_ruled, pieces_with, placements};
use serde_json::{Value, json};
use song::Severity;
use song::pictures::{
    BUDGET, ClipSounds, Cover, MAX_FAULTS, SoundLaneIn, Sounder, Spend, picture_lane_sounds, video_sound_lanes,
};
use song::record::Recorder;
use std::time::{Duration, Instant};

/// Runs `f` and fails when it took fifteen seconds or more (sixty on musl).
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

fn errors(spend: &Spend) -> Vec<&str> {
    spend
        .findings()
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.as_str())
        .collect()
}

fn one_clip() -> song::FamilyRead<song::Clip> {
    clip_family(&[("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"}))])
}

fn disk() -> song::MemSource {
    support::mem(&[("clips/a.wav", "")])
}

fn many(n: usize, f: impl Fn(usize) -> Value) -> Vec<song::Placement> {
    (0..n)
        .map(|i| serde_json::from_value(f(i)).expect("a placement"))
        .collect()
}

#[test]
fn a_cover_of_a_hundred_thousand_spans_in_four_shapes_is_linear() {
    // decision 506: adjacent, nested, identical and each overlapping the next; `+inf` ends
    let n = 100_000_u32;
    type Shape = (&'static str, Box<dyn Fn(u32) -> (f64, f64)>);
    let shapes: Vec<Shape> = vec![
        ("adjacent", Box::new(|i| (f64::from(i), f64::from(i) + 1.0))),
        ("nested", Box::new(move |i| (f64::from(i), f64::from(2 * n - i)))),
        ("identical", Box::new(|_| (5.0, 9.0))),
        ("overlapping", Box::new(|i| (f64::from(i), f64::from(i) + 1.5))),
        ("open", Box::new(|i| (f64::from(i), f64::INFINITY))),
    ];
    for (name, span) in shapes {
        let mut spend = Spend::new();
        let mut cover = Cover::new();
        fast(name, || {
            for i in (0..n).rev() {
                let (a, b) = span(i);
                assert!(cover.hold(a, b, 0.0, &mut spend), "{name}");
            }
        });
        let free = cover.free(-1.0, 1e9, &mut spend).expect("free");
        match name {
            "adjacent" | "nested" | "overlapping" | "open" => {
                assert_eq!(free.first(), Some(&(-1.0, 0.0)), "{name}");
            }
            _ => assert_eq!(free, [(-1.0, 5.0), (9.0, 1e9)]),
        }
        assert!(spend.spent() < 6 * n as usize, "{name} spent {}", spend.spent());
    }
}

#[test]
fn a_hundred_thousand_shots_each_covering_the_last_make_a_hundred_thousand_clips() {
    // decision 506 and 522: each shot keeps the first second of its two
    let evs = many(100_000, |i| json!({"ref": "a", "start": i, "length": 2}));
    let mut spend = Spend::new();
    let shots = own(&evs);
    let out = fast("covering", || {
        pieces_with(&disk(), &one_clip(), 2.0, &shots, &mut spend)
    })
    .expect("pieces");
    assert_eq!(out.len(), 100_000);
    assert!(!spend.capped() && spend.spent() < BUDGET, "spent {}", spend.spent());
    assert_eq!((out[0].shot, out[99_999].shot), (0, 99_999));
}

#[test]
fn a_hundred_thousand_shots_of_which_one_sounds_cost_one_stat() {
    // decisions 511 and 512
    let clips = clip_family(&[
        ("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"})),
        ("mute", json!({"source": "clips/m.mp4"})),
    ]);
    let evs = many(100_000, |i| {
        if i == 50_000 {
            json!({"ref": "a", "start": i, "length": 1})
        } else {
            json!({"ref": "mute", "start": i, "length": 1})
        }
    });
    let src = disk();
    let counting = Counting::new(&src);
    let mut spend = Spend::new();
    let shots = own(&evs);
    let out = fast("one sounds", || pieces_with(&counting, &clips, 1.0, &shots, &mut spend)).expect("pieces");
    assert_eq!((out.len(), counting.stats().len()), (1, 1));
}

#[test]
fn five_thousand_fade_only_shots_over_five_thousand_short_solid_ones_stop_at_the_budget() {
    // decision 523: 25 million pieces from 300 KB, one ERROR, `capped`
    let mut events: Vec<Value> = (0..5_000)
        .map(|_| json!({"ref": "a", "start": 0, "length": 20_000, "fadeIn": 20_000}))
        .collect();
    events.extend((0..5_000).map(|i| json!({"ref": "a", "start": 2 * i, "length": 1})));
    let evs = placements(&events);
    let mut spend = Spend::new();
    let shots = own(&evs);
    let out = fast("fan-out", || pieces_with(&disk(), &one_clip(), 1.0, &shots, &mut spend)).expect("pieces");
    assert!(out.is_empty() && spend.capped());
    assert_eq!(errors(&spend), ["nodes/main.k7x"]);
    assert!(spend.spent() < BUDGET + 10_000);
}

#[test]
fn a_one_megabyte_sound_named_by_a_hundred_thousand_shots_is_stopped_by_its_bytes() {
    // decision 522: each piece writes the path, so it weighs it
    let word = format!("clips/{}.wav", "w".repeat(1 << 20));
    let clips = clip_family(&[("a", json!({"source": "clips/a.mp4", "sound": word}))]);
    let evs = many(100_000, |i| json!({"ref": "a", "start": i, "length": 1}));
    let mut spend = Spend::new();
    let shots = own(&evs);
    let out = fast("bytes", || {
        pieces_with(&support::mem(&[]), &clips, 1.0, &shots, &mut spend)
    })
    .expect("pieces");
    assert!(out.is_empty() && spend.capped());
    assert_eq!(errors(&spend).len(), 1);
}

#[test]
fn a_ref_of_one_megabyte_is_one_piece_with_its_name() {
    // decision 509: the name is the ref, whole
    let long = "r".repeat(1 << 20);
    let clips = clip_family(&[(long.as_str(), json!({"source": "clips/a.mp4", "sound": "clips/a.wav"}))]);
    let evs = placements(&[json!({"ref": long, "start": 0, "length": 1})]);
    let mut spend = Spend::new();
    let out = fast("long ref", || pieces_with(&disk(), &clips, 1.0, &own(&evs), &mut spend)).expect("pieces");
    assert_eq!((out.len(), out[0].clip.name.len()), (1, 1 << 20));
    assert!(!spend.capped());
}

#[test]
fn a_start_near_1e308_at_300_bpm_and_a_speed_of_1e300_are_dropped_with_an_error_and_no_null() {
    // decision 510
    let evs = placements(&[
        json!({"ref": "a", "start": 1e308, "length": 1e308}),
        json!({"ref": "a", "start": 0, "length": 1e308, "speed": 1e300}),
        json!({"ref": "a", "start": 0, "length": 1}),
    ]);
    let mut spend = Spend::new();
    let out = pieces_with(&disk(), &one_clip(), 5.0, &own(&evs), &mut spend).expect("pieces");
    assert!(
        out.iter()
            .all(|p| !serde_json::to_string(&p.clip).expect("json").contains("null"))
    );
    assert_eq!(errors(&spend), ["clips/a.k7x"]);
}

#[test]
fn a_hundred_thousand_activations_with_a_shot_after_each_end_each_shot_at_the_next_activation() {
    // decision 504: the next event's start, an activation included
    let evs = many(200_000, |i| {
        if i % 2 == 0 {
            json!({"start": i / 2})
        } else {
            json!({"ref": "a", "start": i / 2})
        }
    });
    let mut spend = Spend::new();
    let out = fast("activations", || {
        pieces_with(&disk(), &one_clip(), 1.0, &own(&evs), &mut spend)
    })
    .expect("pieces");
    assert_eq!(out.len(), 100_000);
    assert!(out.iter().take(99_999).all(|p| p.clip.length == 1.0));
    assert_eq!(out[99_999].clip.length, 0.0, "the last is open");
}

#[test]
fn a_hundred_thousand_absent_words_list_their_folder_once_and_stop_at_a_thousand_faults() {
    // decisions 154 and 515: through a Recorder, 1,000 faults and `More`
    let items: Vec<(String, Value)> = (0..100_000)
        .map(|i| {
            (
                format!("c{i}"),
                json!({"source": "x.mp4", "sound": format!("clips/s{i}.wav")}),
            )
        })
        .collect();
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let clips = clip_family(&refs);
    let evs = many(100_000, |i| json!({"ref": format!("c{i}"), "start": i, "length": 1}));
    let mem = support::mem(&[("clips/present.wav", "")]);
    let counting = Counting::new(&mem);
    let rec = Recorder::new(&counting);
    let sounds = ClipSounds::of(&clips);
    let mut spend = Spend::new();
    let owned = vec![("main".to_owned(), evs)];
    let shots = lane_shots(&owned);
    let out = fast("absent", || {
        let mut s = Sounder::new(&rec, "/p", 1.0, &sounds, &mut spend);
        picture_lane_sounds(&mut s, &[], &shots, &|_| false, &|id| format!("nodes/{id}.k7x")).expect("own")
    });
    assert!(!out.contains_key("main"), "the walk stopped before it answered");
    assert_eq!(spend.faults().len(), MAX_FAULTS + 1);
    assert_eq!(spend.faults().last().map(|f| f.kind.as_str()), Some("more"));
    assert_eq!(counting.lists().iter().filter(|d| d.as_str() == "/p/clips").count(), 1);
}

#[test]
fn one_word_on_a_hundred_thousand_shots_is_one_stat() {
    // decision 512
    let evs = many(100_000, |i| json!({"ref": "a", "start": i, "length": 1}));
    let src = disk();
    let counting = Counting::new(&src);
    let out = fast("one word", || {
        pieces_with(&counting, &one_clip(), 1.0, &own(&evs), &mut Spend::new())
    })
    .expect("p");
    assert_eq!(
        (out.len(), counting.stats()),
        (100_000, vec!["/p/clips/a.wav".to_owned()])
    );
}

#[test]
fn a_word_that_climbs_names_an_absolute_file_or_holds_a_nul_costs_that_sound_only() {
    // decisions 512 and 514: `../../x.wav`, `/etc/passwd`, a NUL, `a\b.wav` (a character on Unix, a separator on Windows), a folder
    let words = [
        "../../x.wav",
        "/etc/passwd",
        "clips/a\u{0}b.wav",
        "clips\\b.wav",
        "clips",
    ];
    let items: Vec<(String, Value)> = words
        .iter()
        .enumerate()
        .map(|(i, w)| (format!("c{i}"), json!({"source": "x.mp4", "sound": w})))
        .collect();
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let clips = clip_family(&refs);
    let evs = many(5, |i| json!({"ref": format!("c{i}"), "start": i, "length": 1}));
    for (rule, want) in [
        (song::PathRule::Windows, "/p/clips/b.wav"),
        (song::PathRule::Posix, "/p/clips\\b.wav"),
    ] {
        let mut spend = Spend::new();
        let out = pieces_ruled(&disk(), "/p", &clips, 1.0, &own(&evs), &mut spend, rule).expect("pieces");
        let kept: Vec<usize> = out.iter().map(|p| p.shot).collect();
        // The memory Source never refuses a name; only the rule turns a `\` into a separator.
        assert_eq!(kept, vec![3, 4], "{rule:?}");
        assert_eq!(spend.findings().len(), 3, "{rule:?}");
        let backslash = out.iter().find(|p| p.shot == 3).map(|p| p.clip.source.clone());
        assert_eq!(backslash.as_deref(), Some(want), "{rule:?}");
    }
}

#[test]
fn a_drive_letter_is_a_folder_under_the_posix_rule_and_an_absolute_path_under_the_windows_one() {
    // decision 512: `C:/x.wav` joins the project under the Posix rule; under the Windows one it stands alone, outside every mount, so 514 costs that sound alone
    let clips = clip_family(&[("c", json!({"source": "x.mp4", "sound": "C:/x.wav"}))]);
    let evs = many(1, |_| json!({"ref": "c", "start": 0, "length": 1}));
    let src = support::mem(&[("C:/x.wav", "")]);
    let windows = pieces_ruled(
        &src,
        "/p",
        &clips,
        1.0,
        &own(&evs),
        &mut Spend::new(),
        song::PathRule::Windows,
    );
    assert!(windows.expect("a refused word costs that sound alone").is_empty());
    let posix = pieces_ruled(
        &src,
        "/p",
        &clips,
        1.0,
        &own(&evs),
        &mut Spend::new(),
        song::PathRule::Posix,
    );
    let sources: Vec<String> = posix.expect("pieces").into_iter().map(|p| p.clip.source).collect();
    assert_eq!(sources, ["/p/C:/x.wav"]);
}

#[test]
fn a_link_out_of_the_mounts_and_a_dangling_link_warn_and_a_folder_counts() {
    // decision 514: the disk Source answers Outside for both links
    let t = support::Tmp::new("pictures-links");
    let outside = support::Tmp::new("pictures-outside");
    outside.write("x.wav", "");
    t.write("ws/proj/clips/a.wav", "");
    t.mkdir("ws/proj/clips/dir");
    let ok = link(&outside.0.join("x.wav"), &t.0.join("ws/proj/clips/out.wav"))
        && link(
            &t.0.join("ws/proj/clips/none.wav"),
            &t.0.join("ws/proj/clips/dangling.wav"),
        );
    if !ok {
        support::skip("a_link_out_of_the_mounts", "this machine does not allow links");
        return;
    }
    let src = song::DiskSource::new(&[t.at("ws")]).expect("source");
    let clips = clip_family(&[
        ("out", json!({"source": "x.mp4", "sound": "clips/out.wav"})),
        ("dangling", json!({"source": "x.mp4", "sound": "clips/dangling.wav"})),
        ("dir", json!({"source": "x.mp4", "sound": "clips/dir"})),
    ]);
    let refs = ["out", "dangling", "dir"];
    let evs = many(3, |i| json!({"ref": refs[i], "start": i, "length": 1}));
    let mut spend = Spend::new();
    let out = pieces_in(&src, &t.at("ws/proj"), &clips, 1.0, &own(&evs), &mut spend).expect("pieces");
    assert_eq!(out.iter().map(|p| p.shot).collect::<Vec<_>>(), [2]);
    assert_eq!(spend.findings().len(), 2);
}

fn link(target: &std::path::Path, at: &std::path::Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, at).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(target, at).is_ok();
    #[cfg(not(any(unix, windows)))]
    return false;
}

fn lane_in<'a>(name: &'a str, from: Option<&'a str>, events: usize) -> SoundLaneIn<'a> {
    SoundLaneIn {
        name,
        events,
        video_source: from,
        file: "nodes/lane.k7x".to_owned(),
    }
}

#[test]
fn a_hundred_thousand_lanes_naming_one_lane_of_a_hundred_thousand_shots_stop_at_the_budget() {
    // decisions 516 and 523
    let evs = many(100_000, |i| json!({"ref": "a", "start": i, "length": 1}));
    let owned = vec![("main".to_owned(), evs)];
    let shots = lane_shots(&owned);
    let names: Vec<String> = (0..100_000).map(|i| format!("l{i}")).collect();
    let lanes: Vec<SoundLaneIn<'_>> = names.iter().map(|n| lane_in(n, Some("main"), 0)).collect();
    let (clips, src) = (one_clip(), disk());
    let sounds = ClipSounds::of(&clips);
    let mut spend = Spend::new();
    let out = fast("lanes", || {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        video_sound_lanes(&mut s, &lanes, &shots).expect("lanes")
    });
    assert!(spend.capped() && out.len() <= 20);
    assert_eq!(errors(&spend).len(), 1);
}

#[test]
fn a_hundred_thousand_lanes_naming_nothing_are_a_thousand_faults_and_more() {
    // decisions 515 and 516: each string under 200 characters, each message under 300 bytes
    let long = "q".repeat(1 << 9);
    let names: Vec<String> = (0..100_000).map(|i| format!("{i}-{long}")).collect();
    let lanes: Vec<SoundLaneIn<'_>> = names.iter().map(|n| lane_in(n, Some("nope"), 0)).collect();
    let (clips, src) = (one_clip(), disk());
    let sounds = ClipSounds::of(&clips);
    let mut spend = Spend::new();
    let shots = lane_shots(&[]);
    fast("nothing", || {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        video_sound_lanes(&mut s, &lanes, &shots).expect("lanes")
    });
    let (findings, faults) = spend.into_parts();
    assert_eq!(faults.len(), MAX_FAULTS + 1);
    assert!(
        faults
            .iter()
            .all(|f| f.holder.chars().count() <= 200 && f.target.chars().count() <= 200)
    );
    assert!(findings.iter().all(|f| f.message.len() <= 300));
}

#[test]
fn a_hundred_thousand_picture_lanes_and_a_hundred_thousand_video_sources_on_one_are_linear() {
    // decision 517: `sounded` and `wired` are sets built once, asked per lane
    let names: Vec<String> = (0..100_000).map(|i| format!("p{i}")).collect();
    let owned: Vec<(String, Vec<song::Placement>)> = names.iter().map(|n| (n.clone(), Vec::new())).collect();
    let shots = lane_shots(&owned);
    let wired: std::collections::HashSet<&str> = names.iter().map(String::as_str).step_by(2).collect();
    let lanes: Vec<SoundLaneIn<'_>> = (0..100_000).map(|_| lane_in("x", Some("p7"), 0)).collect();
    let (clips, src) = (one_clip(), disk());
    let sounds = ClipSounds::of(&clips);
    let mut spend = Spend::new();
    let asked = std::cell::Cell::new(0_usize);
    let out = fast("picture lanes", || {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        picture_lane_sounds(
            &mut s,
            &lanes,
            &shots,
            &|id| {
                asked.set(asked.get() + 1);
                wired.contains(id)
            },
            &|id| format!("nodes/{id}.k7x"),
        )
        .expect("own")
    });
    // Every second lane is wired, and "p7" is sounded by the lanes, so it is not asked at all.
    assert_eq!(out.len(), 50_000);
    assert_eq!(asked.get(), 99_999);
    assert!(!spend.capped());
}

#[test]
fn a_one_megabyte_video_source_is_a_fault_cut_short() {
    // decisions 515 and 516
    let long = "v".repeat(1 << 20);
    let lanes = [lane_in("x", Some(&long), 0)];
    let (clips, src) = (one_clip(), disk());
    let sounds = ClipSounds::of(&clips);
    let mut spend = Spend::new();
    {
        let mut s = Sounder::new(&src, "/p", 1.0, &sounds, &mut spend);
        video_sound_lanes(&mut s, &lanes, &lane_shots(&[])).expect("lanes");
    }
    assert_eq!(spend.faults()[0].target.chars().count(), 200);
    assert!(spend.findings()[0].message.len() <= 300);
}
