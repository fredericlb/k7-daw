// clips_hostile.rs — hostile input to every pass of the placements, each refused fast and asserted on its answer as well as its time.
// A reader would get wrong that the row counts the OUTPUT: a 60 KB project can name 10^12 clips, and none is made past the budget.

use crate::clips_support;
use crate::support;

use clips_support::{clip_family, event, grid, paths, world};
use serde_json::{Value, json};
use song::clips::{
    BUDGET, Budget, ClipDoc, CompSpan, Materials, Piece, Placer, SeamFade, TakeRef, audio_paths, comp_pieces,
    mark_crossfades, seam_crossfades,
};
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

fn expand(m: &Materials, events: Vec<Value>) -> (usize, Vec<song::Finding>, bool) {
    let evs: Vec<song::Placement> = events.into_iter().map(event).collect();
    let mut placer = Placer::new(grid(120.0), m);
    let leaves = placer.expand_events(&evs, "nodes/song.k7x");
    let n = leaves.len();
    let (findings, capped, _) = placer.finish();
    (n, findings, capped)
}

fn events_of(r: &str, n: usize) -> Value {
    Value::Array((0..n).map(|_| json!({"ref": r, "start": "1:1"})).collect())
}

#[test]
fn six_levels_of_a_hundred_events_are_ten_to_the_twelve_leaves_and_stop_at_the_budget() {
    // decision 157
    let mut tl: Vec<(String, Value)> = (0..5)
        .map(|i| {
            (
                format!("t{i}"),
                json!({"length": "1:0", "lanes": [{"lane": "x", "events": events_of(&format!("t{}", i + 1), 100)}]}),
            )
        })
        .collect();
    tl.push((
        "t5".into(),
        json!({"length": "1:0", "lanes": [{"lane": "x", "events": events_of("kick", 100)}]}),
    ));
    let refs: Vec<(&str, Value)> = tl.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[("kick", json!({"source": "k.wav"}))], &[], &refs);
    let (n, findings, capped) = fast("six levels", || expand(&m, vec![json!({"ref": "t0", "start": "1:1"})]));
    assert!(capped && n <= BUDGET, "{n} leaves");
    assert_eq!(findings.len(), 1);
    assert!(findings[0].file.starts_with("timelines/t"));
}

#[test]
fn a_chain_of_a_hundred_thousand_timelines_is_cut_at_the_depth_without_overflowing_the_stack() {
    // decision 158
    let tl: Vec<(String, Value)> = (0..100_000)
        .map(|i| {
            (
                format!("t{i}"),
                json!({"lanes": [{"lane": "x", "events": [{"ref": format!("t{}", i + 1), "start": "1:1"}]}]}),
            )
        })
        .collect();
    let refs: Vec<(&str, Value)> = tl.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[], &[], &refs);
    let (n, findings, capped) = fast("a chain", || expand(&m, vec![json!({"ref": "t0", "start": "1:1"})]));
    assert_eq!((n, capped), (0, false));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "timelines/t32.k7x");
}

#[test]
fn a_timeline_in_itself_and_two_in_a_cycle_are_cut_without_a_finding() {
    // decision 158: a timeline already on the path is cut silently, and the reference check reports it
    let m = world(
        &[],
        &[],
        &[(
            "self",
            json!({"lanes": [{"lane": "x", "events": events_of("self", 100)}]}),
        )],
    );
    assert_eq!(
        fast("self", || expand(&m, vec![json!({"ref": "self", "start": "1:1"})])),
        (0, vec![], false)
    );
    let cycle = |n: usize| {
        world(
            &[],
            &[],
            &[
                ("a", json!({"lanes": [{"lane": "x", "events": events_of("b", n)}]})),
                ("b", json!({"lanes": [{"lane": "x", "events": events_of("a", n)}]})),
            ],
        )
    };
    let start = || vec![json!({"ref": "a", "start": "1:1"})];
    let m = cycle(10);
    assert_eq!(fast("cycle", || expand(&m, start())), (0, vec![], false));
    // decision 157: a cycle has no memo, so a hundred events each way walks past the budget and is capped
    let m = cycle(100);
    let (n, findings, capped) = fast("big cycle", || expand(&m, start()));
    assert!((n, findings.len(), capped) == (0, 1, true));
}

#[test]
fn a_fan_out_of_forty_levels_over_a_cycle_weighs_its_visits() {
    // decision 157, 159: two events per level and a cycle at the bottom, so nothing is ever memoised
    let tl: Vec<(String, Value)> = (0..40)
        .map(|i| {
            let next = if i < 39 { format!("t{}", i + 1) } else { "t0".to_owned() };
            (
                format!("t{i}"),
                json!({"lanes": [{"lane": "x", "events": events_of(&next, 2)}]}),
            )
        })
        .collect();
    let refs: Vec<(&str, Value)> = tl.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[], &[], &refs);
    let mut placer = Placer::new(grid(120.0), &m);
    let len = fast("fan-out", || placer.timeline_length("t0"));
    assert!(len.is_finite());
    let (findings, capped, spent) = placer.finish();
    assert!(capped && spent > BUDGET);
    assert_eq!(findings.len(), 1);
}

#[test]
fn a_zero_zero_length_is_zero_and_a_four_hundred_digit_bar_has_no_view() {
    // decision 156 on a timeline: its length is not finite, so it has no TimelineDoc and an error on its file
    let digits = format!("{}:0", "9".repeat(400));
    let m = world(
        &[],
        &[],
        &[
            ("huge", json!({"length": digits, "lanes": [{"lane": "x"}]})),
            ("none", json!({"length": "0:0", "lanes": [{"lane": "x"}]})),
        ],
    );
    let mut placer = Placer::new(grid(120.0), &m);
    let docs = fast("bar", || placer.timeline_docs());
    assert_eq!(
        docs.iter().map(|d| (d.id.as_str(), d.length)).collect::<Vec<_>>(),
        [("none", 0.0)]
    );
    let (findings, _, _) = placer.finish();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "timelines/huge.k7x");
}

#[test]
fn a_sequence_of_a_hundred_thousand_notes_placed_a_thousand_times_is_not_cloned_past_the_budget() {
    // decision 157: the notes weigh the clip BEFORE the copy
    let notes: Vec<Value> = (0..100_000)
        .map(|i| json!({"t": f64::from(i) * 0.001, "d": 0.001, "k": 60}))
        .collect();
    let m = world(&[], &[("big", json!({ "notes": notes }))], &[]);
    let mut placer = Placer::new(grid(120.0), &m);
    let e = event(json!({"ref": "big", "start": "1:1"}));
    let made = fast("big sequence", || {
        (0..1000)
            .filter(|_| placer.placement_to_clip(&e, "nodes/x.k7x").is_some())
            .count()
    });
    assert!((9..=10).contains(&made), "{made} placed");
    let (findings, capped, spent) = placer.finish();
    assert!(capped && spent <= BUDGET + 100_001);
    assert_eq!(findings.len(), 1);
}

fn piece(r: &str, start: f64, length: f64) -> Piece {
    Piece {
        ref_: r.into(),
        start,
        offset: 0.0,
        length: Some(length),
    }
}

#[test]
fn a_hundred_thousand_comp_spans_are_past_the_budget() {
    // decision 162
    let spans: Vec<CompSpan> = (0..100_000)
        .map(|i| CompSpan {
            take: "t".into(),
            from: f64::from(i) * 2.0,
            to: f64::from(i) * 2.0 + 1.0,
        })
        .collect();
    let takes = [TakeRef {
        ref_: "t".into(),
        start: 0.0,
    }];
    let mut b = Budget::new(BUDGET);
    let got = fast("comp spans", || {
        comp_pieces(&piece("gtr", 0.0, 400_000.0), &takes, &spans, &mut b)
    });
    assert!(got.is_none() && b.capped());
}

#[test]
fn a_hundred_thousand_crossfades_over_a_thousand_pieces_are_past_the_budget() {
    // decision 162
    let pieces: Vec<Piece> = (0..1000).map(|i| piece("p", f64::from(i) * 4.0, 4.0)).collect();
    let xfades: Vec<SeamFade> = (0..100_000).map(|_| SeamFade { at: 4.0, length: 0.5 }).collect();
    let mut b = Budget::new(BUDGET);
    let got = fast("crossfades", || {
        seam_crossfades(&pieces, &xfades, &|_| Some(8.0), &mut b)
    });
    assert!(got.is_none() && b.capped());
}

#[test]
fn a_take_list_naming_one_ref_a_hundred_thousand_times_answers_inside_the_budget() {
    // decision 162: the event's words weigh the clip, and one ref is one clip
    let takes: Vec<Value> = (0..100_000).map(|_| json!({"ref": "t", "start": "1:1"})).collect();
    let m = world(
        &[
            ("g", json!({"source": "g.wav", "to": 4})),
            ("t", json!({"source": "t.wav", "to": 4})),
        ],
        &[],
        &[],
    );
    let p = json!({"ref": "g", "start": "1:1", "takes": takes, "comp": [{"take": "t", "from": "1:2", "to": "1:3"}]});
    let clips = fast("takes", || clips_support::lane(&m, 120.0, &[p]));
    assert_eq!(
        clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["g", "t", "g"]
    );
}

fn faded(i: usize, start: f64, fade_in: f64, fade_out: f64) -> ClipDoc {
    let mut c: ClipDoc = serde_json::from_value(json!({
        "id": format!("c{i}"), "type": "audio", "name": "c", "source": "", "start": start, "length": 8, "offset": 0,
        "fadeIn": fade_in, "fadeOut": fade_out, "loopStart": 0, "loopEnd": 0, "notes": []
    }))
    .expect("a clip");
    c.id = format!("c{i}");
    c
}

#[test]
fn a_hundred_thousand_clips_overlapping_by_their_fades_are_marked_in_linear_time() {
    // decision 162
    let clips: Vec<ClipDoc> = (0..100_000_u32)
        .map(|i| faded(i as usize, f64::from(i) * 7.0, 1.0, 1.0))
        .collect();
    let mut b = Budget::new(BUDGET);
    let got = fast("overlaps", || mark_crossfades(clips, &mut b));
    assert!(!b.capped());
    assert_eq!(
        got.iter()
            .filter(|c| c.crossfade.is_some_and(|m| m.out == Some(true)))
            .count(),
        99_999
    );
    assert_eq!(
        got.iter()
            .filter(|c| c.crossfade.is_some_and(|m| m.in_ == Some(true)))
            .count(),
        99_999
    );
}

#[test]
fn fifty_thousand_clips_at_one_start_each_matching_fifty_thousand_at_another_are_past_the_budget() {
    // decision 162: 2.5 x 10^9 pairs, stopped
    let mut clips: Vec<ClipDoc> = (0..50_000).map(|i| faded(i, 0.0, 0.0, 1.0)).collect();
    clips.extend((0..50_000).map(|i| faded(i, 7.0, 1.0, 0.0)));
    let mut b = Budget::new(BUDGET);
    let got = fast("pairs", || mark_crossfades(clips, &mut b));
    assert!(b.capped());
    assert_eq!(got.len(), 100_000);
}

#[test]
fn audio_paths_refuse_what_leaves_the_mounts_and_cost_one_stat_per_clip() {
    // decision 154: a climb, an absolute path, a NUL and a backslash; 100,000 clips
    let src = support::mem(&[("clips/a.wav", "RIFF")]);
    let mut items: Vec<(String, Value)> = vec![
        ("up".into(), json!({"source": "../../x.wav"})),
        ("abs".into(), json!({"source": "/etc/passwd"})),
        ("nul".into(), json!({"source": "a\0.wav"})),
        ("back".into(), json!({"source": "..\\..\\x.wav"})),
    ];
    items.extend((0..100_000).map(|i| (format!("c{i}"), json!({"source": "clips/a.wav"}))));
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let clips = clip_family(&refs);
    assert_eq!(paths(&clips).len(), 100_004);
    let got = fast("paths", || audio_paths(&src, "/p", &clips, song::PathRule::Posix)).expect("paths");
    assert_eq!(
        (got["up"].as_str(), got["abs"].as_str(), got["nul"].as_str()),
        ("", "", "")
    );
    assert_eq!(got["back"], "/p/..\\..\\x.wav");
    let windows = audio_paths(&src, "/p", &clips, song::PathRule::Windows).expect("paths");
    assert_eq!(windows["back"], "");
    assert!(got.iter().skip(4).all(|(_, v)| v == "/p/clips/a.wav"));
}

#[test]
fn a_loop_near_two_to_the_53_and_a_huge_window_never_leave_a_number_that_is_not_finite() {
    // decision 156: the count is a whole number past what a window can hold twice, or the clip is refused with an error
    let big = (1_u64 << 53) - 1;
    // `to: 1e308` loops past the largest number: refused with one error; no window has no length to repeat, so it is finite
    for (clip, refused) in [
        (json!({"source": "k.wav", "to": 1.0e308}), true),
        (json!({"source": "k.wav"}), false),
    ] {
        let m = world(&[("k", clip)], &[], &[]);
        let mut placer = Placer::new(grid(300.0), &m);
        let e = event(json!({"ref": "k", "start": "1:1", "loop": big}));
        let got = placer.placement_to_clip(&e, "nodes/x.k7x");
        let (findings, _, _) = placer.finish();
        if refused {
            assert!(got.is_none() && findings.len() == 1);
        } else {
            assert!(got.as_ref().is_some_and(ClipDoc::finite) && findings.is_empty());
        }
    }
}

#[test]
fn a_comped_event_pays_for_the_words_each_piece_copies() {
    // decision 157: 20,000 takes and 200 contiguous spans are 4,000,000 words copied onto the pieces
    let takes: Vec<Value> = (0..20_000).map(|_| json!({"ref": "t", "start": "1:1"})).collect();
    let comp: Vec<Value> = (0..200)
        .map(|i| json!({"take": "t", "from": format!("{}:1", i + 1), "to": format!("{}:1", i + 2)}))
        .collect();
    let m = world(
        &[
            ("g", json!({"source": "g.wav", "to": 4000})),
            ("t", json!({"source": "t.wav", "to": 4000})),
        ],
        &[],
        &[],
    );
    let p = event(json!({"ref": "g", "start": "1:1", "takes": takes, "comp": comp}));
    let mut placer = Placer::new(grid(120.0), &m);
    let leaves = placer.expand_events(std::slice::from_ref(&p), "nodes/song.k7x");
    let made: usize = fast("comped words", || {
        leaves
            .iter()
            .map(|l| placer.place(l).iter().map(ClipDoc::weight).sum::<usize>())
            .sum()
    });
    let (findings, capped, _) = placer.finish();
    assert!(capped && made <= BUDGET, "{made} words made");
    assert_eq!(findings.len(), 1);
}

#[test]
fn a_fan_out_over_a_cycle_whose_last_timeline_holds_pictures_weighs_the_pictures() {
    // decision 157, 159: twelve levels of two events over a cycle, 2,000 pictures at the bottom
    let pictures: Vec<Value> = (0..2000)
        .map(|_| json!({"ref": "x", "start": "1:1", "length": 1}))
        .collect();
    let tl: Vec<(String, Value)> = (0..12)
        .map(|i| {
            let next = if i < 11 { format!("t{}", i + 1) } else { "t0".to_owned() };
            let mut t = json!({"lanes": [{"lane": "x", "events": events_of(&next, 2)}]});
            if i == 11 {
                t["pictures"] = json!([{"lane": "p", "events": pictures}]);
            }
            (format!("t{i}"), t)
        })
        .collect();
    let refs: Vec<(&str, Value)> = tl.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[], &[], &refs);
    let mut placer = Placer::new(grid(120.0), &m);
    let len = fast("pictures", || placer.timeline_length("t0"));
    assert!(len.is_finite());
    let (findings, capped, _) = placer.finish();
    assert!(capped);
    assert_eq!(findings.len(), 1);
}

#[test]
fn the_bytes_of_a_nested_clips_id_are_weighed() {
    // decision 157: 30 timelines named by 200 characters, 64 instances of 1,000 clips, are 64,000 ids of 6 KB
    let name = |i: usize| format!("{}{i:02}", "n".repeat(200));
    let mut tl: Vec<(String, Value)> = (0..29)
        .map(|i| {
            let lanes = json!([{"lane": "x", "events": events_of(&name(i + 1), 1)}]);
            (name(i), json!({"length": "1:0", "lanes": lanes}))
        })
        .collect();
    let lanes = json!([{"lane": "x", "events": events_of("leaf", 64)}]);
    tl.push((name(29), json!({"length": "1:0", "lanes": lanes})));
    let lanes = json!([{"lane": "x", "events": events_of("kick", 1000)}]);
    tl.push(("leaf".into(), json!({"length": "1:0", "lanes": lanes})));
    let refs: Vec<(&str, Value)> = tl.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[("kick", json!({"source": "k.wav"}))], &[], &refs);
    let mut placer = Placer::new(grid(120.0), &m);
    let first = event(json!({"ref": name(0), "start": "1:1"}));
    let leaves = placer.expand_events(std::slice::from_ref(&first), "nodes/song.k7x");
    let bytes: usize = fast("long ids", || {
        leaves
            .iter()
            .map(|l| placer.place(l).iter().map(|c| c.id.len()).sum::<usize>())
            .sum()
    });
    let (findings, capped, _) = placer.finish();
    assert!(capped && bytes < 100_000_000, "{bytes} bytes of ids");
    assert_eq!(findings.len(), 1);
}
