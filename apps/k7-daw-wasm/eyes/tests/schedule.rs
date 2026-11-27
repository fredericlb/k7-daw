// schedule.rs — the transition rule, asserted without any GL: which clip covers which, and the shapes a frame takes.
// A reader would get wrong that these are `timeline.test.ts`'s cases: the rule lived there until the schedule replaced it.

use eyes::schedule::{LayerSchedule, schedule};
use eyes::{Fade, SceneClip};

fn clip(start: f64, length: f64, id: &str) -> SceneClip {
    SceneClip {
        id: id.into(),
        src: format!("{id}.mp4"),
        start,
        length,
        ..SceneClip::default()
    }
}

fn named(start: f64, length: f64) -> SceneClip {
    clip(start, length, &format!("c{start}"))
}

fn with(mut c: SceneClip, fade_in: Option<Fade>, fade_out: Option<Fade>) -> SceneClip {
    c.fade_in = fade_in;
    c.fade_out = fade_out;
    c
}

fn fade(over: f64, with_: &str, chain: Option<&str>) -> Fade {
    Fade {
        over,
        with_: with_.into(),
        chain: chain.map(str::to_owned),
    }
}

fn dissolve(over: f64) -> Fade {
    fade(over, "dissolve", None)
}

/// What the page reads at `t`: the two ids, and the blend's name, progress and each stretch's chain and presence.
#[derive(Debug, PartialEq)]
struct Frame {
    from: Option<String>,
    to: Option<String>,
    blend: Option<(String, f64, Option<(String, f64)>, Option<(String, f64)>)>,
}

fn frame(clips: &[SceneClip], s: &LayerSchedule, t: f64) -> Option<Frame> {
    let sp = s.span_at(t)?;
    let id = |i: Option<u32>| i.map(|i| clips[i as usize].id.clone());
    let blend = sp.blend.as_ref().map(|b| {
        let st = |x: &Option<eyes::schedule::Stretch>| x.as_ref().map(|x| (x.chain.clone(), x.presence.at(t)));
        (b.name.clone(), b.progress.at(t), st(&b.stretch_from), st(&b.stretch_to))
    });
    Some(Frame {
        from: id(sp.from),
        to: id(sp.to),
        blend,
    })
}

fn top(clips: &[SceneClip], t: f64) -> Option<String> {
    schedule(clips).span_at(t).map(|s| clips[s.top as usize].id.clone())
}

fn progress(clips: &[SceneClip], t: f64) -> f64 {
    frame(clips, &schedule(clips), t).unwrap().blend.unwrap().1
}

#[test]
fn nothing_is_on_before_between_and_after() {
    let c = [named(2.0, 2.0), named(6.0, 2.0)];
    let s = schedule(&c);
    for t in [0.0, 1.9, 4.0, 5.9, 8.0, 99.0] {
        assert!(s.span_at(t).is_none(), "{t}");
    }
}

#[test]
fn the_top_shot_of_an_overlap_is_the_last_in_file_order() {
    // The first would make a completed dissolve snap back to the outgoing shot for its last frames.
    let c = [named(0.0, 5.0), named(4.0, 5.0)];
    assert_eq!(top(&c, 4.5).as_deref(), Some("c4"));
    assert_eq!(top(&c, 4.99).as_deref(), Some("c4"));
    assert_eq!(top(&[], 0.0), None);
    assert_eq!(top(&[named(0.0, 1.0), named(4.0, 1.0)], 2.0), None);
}

#[test]
fn a_lane_has_two_decoders_so_the_two_on_top_win() {
    let c = [
        named(0.0, 9.0),
        named(1.0, 9.0),
        with(named(2.0, 9.0), Some(dissolve(9.0)), None),
    ];
    let f = frame(&c, &schedule(&c), 5.0).unwrap();
    assert_eq!((f.from.as_deref(), f.to.as_deref()), (Some("c1"), Some("c2")));
}

#[test]
fn a_length_0_shot_runs_to_the_next_start_and_the_last_one_for_ever() {
    let c = [named(0.0, 0.0), named(4.0, 0.0)];
    assert_eq!(top(&c, 3.9).as_deref(), Some("c0"));
    assert_eq!(top(&c, 1e6).as_deref(), Some("c4"));
    let s = schedule(&c);
    assert_eq!(s.ends, vec![Some(4.0), None]);
}

#[test]
fn a_plain_cut_when_nothing_fades() {
    let c = [named(0.0, 4.0)];
    let f = frame(&c, &schedule(&c), 1.0).unwrap();
    assert_eq!((f.blend, f.to.as_deref(), f.from), (None, Some("c0"), None));
}

#[test]
fn the_incoming_shot_blends_over_the_outgoing_one_0_to_1() {
    let c = [named(0.0, 5.0), with(clip(4.0, 5.0, "c4"), Some(dissolve(1.0)), None)];
    assert_eq!(progress(&c, 4.0), 0.0);
    assert!((progress(&c, 4.5) - 0.5).abs() < 1e-9);
    let mid = frame(&c, &schedule(&c), 4.5).unwrap();
    assert_eq!((mid.from.as_deref(), mid.to.as_deref()), (Some("c0"), Some("c4")));
}

#[test]
fn the_incoming_shot_draws_alone_once_the_window_is_over_with_no_snap_back() {
    // The outgoing shot OUTLASTS the fade: picking the first active would blit c0 again.
    let c = [named(0.0, 6.0), with(clip(4.0, 5.0, "c4"), Some(dissolve(1.0)), None)];
    let s = schedule(&c);
    assert!(frame(&c, &s, 4.9).unwrap().blend.is_some());
    let f = frame(&c, &s, 5.5).unwrap();
    assert_eq!((f.blend, f.to.as_deref()), (None, Some("c4")));
}

#[test]
fn a_fade_in_from_nothing_is_the_same_branch() {
    let c = [with(named(0.0, 5.0), Some(dissolve(2.0)), None)];
    let f = frame(&c, &schedule(&c), 1.0).unwrap();
    assert_eq!((f.from, f.to.as_deref()), (None, Some("c0")));
    assert!((f.blend.unwrap().1 - 0.5).abs() < 1e-9);
}

#[test]
fn a_fade_out_goes_toward_nothing_over_the_shot_s_own_end() {
    let c = [with(named(0.0, 4.0), None, Some(dissolve(2.0)))];
    let s = schedule(&c);
    assert_eq!(frame(&c, &s, 1.0).unwrap().blend, None);
    let f = frame(&c, &s, 3.0).unwrap();
    assert_eq!((f.from.as_deref(), f.to.as_deref()), (Some("c0"), None));
    assert!((f.blend.unwrap().1 - 0.5).abs() < 1e-9);
}

#[test]
fn a_length_0_shot_s_fade_out_anchors_on_the_next_shot_s_start() {
    let c = [with(named(0.0, 0.0), None, Some(dissolve(2.0))), named(8.0, 2.0)];
    assert!((progress(&c, 7.0) - 0.5).abs() < 1e-9);
}

#[test]
fn an_open_ended_shot_runs_no_fade_out() {
    let c = [with(named(0.0, 0.0), None, Some(dissolve(2.0)))];
    assert_eq!(frame(&c, &schedule(&c), 1e6).unwrap().blend, None);
}

#[test]
fn a_fade_longer_than_the_shot_is_clamped_rather_than_refused() {
    let c = [with(named(0.0, 1.0), Some(dissolve(10.0)), None)];
    assert_eq!(progress(&c, 0.0), 0.0);
    assert!((progress(&c, 0.5) - 0.5).abs() < 1e-9);
    assert!(progress(&c, 0.99) < 1.0);
}

#[test]
fn the_named_kind_is_carried_through() {
    let c = [with(named(0.0, 4.0), Some(fade(1.0, "wipe", None)), None)];
    assert_eq!(frame(&c, &schedule(&c), 0.5).unwrap().blend.unwrap().0, "wipe");
}

#[test]
fn fade_in_wins_over_fade_out_when_a_shot_is_short_enough_to_run_both() {
    let c = [with(named(0.0, 2.0), Some(dissolve(2.0)), Some(dissolve(2.0)))];
    let f = frame(&c, &schedule(&c), 1.5).unwrap();
    assert_eq!((f.to.as_deref(), f.from), (Some("c0"), None));
}

fn over(chain: &str, s: f64) -> Fade {
    fade(s, "over", Some(chain))
}

#[test]
fn a_stretch_runs_the_incoming_shot_through_it_presence_rising() {
    let c = [
        named(0.0, 4.0),
        with(clip(3.0, 4.0, "c3"), Some(over("iris", 1.0)), None),
    ];
    let b = frame(&c, &schedule(&c), 3.25).unwrap().blend.unwrap();
    assert_eq!((b.0.as_str(), b.1), ("over", 0.25));
    assert_eq!((b.2, b.3), (None, Some(("iris".to_owned(), 0.25))));
}

#[test]
fn a_stretch_runs_the_leaving_shot_through_its_fade_out_presence_falling() {
    let c = [with(named(0.0, 4.0), None, Some(over("zoom", 2.0)))];
    let b = frame(&c, &schedule(&c), 3.5).unwrap().blend.unwrap();
    assert_eq!((b.2, b.3), (Some(("zoom".to_owned(), 0.25)), None));
}

#[test]
fn an_overlap_runs_both_the_incoming_blends_and_the_outgoing_leaves_its_own_way() {
    let c = [
        with(named(0.0, 4.0), None, Some(over("zoom", 1.0))),
        with(clip(3.0, 4.0, "c3"), Some(over("iris", 1.0)), None),
    ];
    let b = frame(&c, &schedule(&c), 3.5).unwrap().blend.unwrap();
    assert_eq!(b.2, Some(("zoom".to_owned(), 0.5)));
    assert_eq!(b.3, Some(("iris".to_owned(), 0.5)));
}

#[test]
fn the_outgoing_shot_is_left_alone_before_its_own_fade_out_window_opens() {
    let c = [
        with(named(0.0, 4.0), None, Some(over("zoom", 0.5))),
        with(clip(3.0, 4.0, "c3"), Some(over("iris", 1.0)), None),
    ];
    let b = frame(&c, &schedule(&c), 3.25).unwrap().blend.unwrap();
    assert_eq!((b.2, b.3), (None, Some(("iris".to_owned(), 0.25))));
}

#[test]
fn a_plain_dissolve_carries_no_stretch() {
    let c = [named(0.0, 4.0), with(clip(3.0, 4.0, "c3"), Some(dissolve(1.0)), None)];
    let b = frame(&c, &schedule(&c), 3.5).unwrap().blend.unwrap();
    assert_eq!((b.0.as_str(), b.1, b.2, b.3), ("dissolve", 0.5, None, None));
}

#[test]
fn a_shape_lane_s_activation_spans_say_when_it_is_on() {
    // An activation carries no picture, so it has no src and no in-point: only when the lane is on.
    let on = |s, l| SceneClip {
        start: s,
        length: l,
        ..SceneClip::default()
    };
    let c = [on(2.0, 2.0), on(6.0, 2.0)];
    let s = schedule(&c);
    for t in [0.0, 1.9, 4.0, 5.9, 8.0, 99.0] {
        assert!(s.span_at(t).is_none(), "{t}");
    }
    for t in [2.0, 3.9, 6.0, 7.9] {
        assert!(s.span_at(t).is_some(), "{t}");
    }
    let open = schedule(&[on(0.0, 0.0), on(4.0, 0.0)]);
    assert!(open.span_at(3.9).is_some() && open.span_at(1e6).is_some());
}

#[test]
fn a_cut_between_back_to_back_shots_is_one_span_per_shot() {
    let c = [named(0.0, 2.0), named(2.0, 2.0)];
    let s = schedule(&c);
    assert_eq!(s.spans.len(), 2);
    assert_eq!(s.span_at(1.999).unwrap().top, 0);
    assert_eq!(s.span_at(2.0).unwrap().top, 1);
}

#[test]
fn a_schedule_is_small_next_to_its_clips() {
    // A hundred shots, every other one dissolving: a few spans each, never a sample per frame.
    let c: Vec<SceneClip> = (0..100)
        .map(|i| {
            let f = (i % 2 == 1).then(|| dissolve(0.5));
            with(clip(i as f64 * 2.0, 2.5, &format!("s{i}")), f, None)
        })
        .collect();
    assert!(schedule(&c).spans.len() < 500);
}
