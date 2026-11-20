// curves_baker.rs — the bake of one compile: the note-ons a lane lays, each curve resolved once, and what a refusal leaves; no TS test.
// A reader would get wrong that a None is not an Err: no such curve, no point to play, a refusal and a cap all answer None.

use crate::curves_support;
use crate::support;

use curves_support::{file, pairs, sig};
use indexmap::IndexMap;
use song::Cache;
use song::curves::{CurveBaker, CurveFile, MidiWindow, read_curves};
use std::sync::Arc;

fn curves(files: &[(&str, &str)]) -> IndexMap<String, Arc<CurveFile>> {
    files
        .iter()
        .map(|(id, text)| ((*id).to_owned(), Arc::new(file(text))))
        .collect()
}

fn window(start: f64, length: f64, loop_start: f64, loop_end: f64, notes: &[f64]) -> (f64, f64, f64, f64, Vec<f64>) {
    (start, length, loop_start, loop_end, notes.to_vec())
}

fn lay(clips: &[(f64, f64, f64, f64, Vec<f64>)]) -> Vec<f64> {
    let map = IndexMap::new();
    let mut baker = CurveBaker::new(&map, sig(4, 4), 120.0, 32.0);
    let windows: Vec<MidiWindow> = clips
        .iter()
        .map(|c| MidiWindow {
            start: c.0,
            length: c.1,
            loop_start: c.2,
            loop_end: c.3,
            note_times: &c.4,
        })
        .collect();
    baker.note_ons(&windows)
}

// bakeAutomations' noteOns: not looped, a note before the clip's end lands at the clip's start plus its time.
#[test]
fn a_plain_clip_lays_the_notes_before_its_end() {
    assert_eq!(
        lay(&[window(8.0, 4.0, 0.0, 0.0, &[0.0, 1.0, 3.9, 4.0, 9.0])]),
        [8.0, 9.0, 11.9]
    );
    // A length of 0 is the clip's whole length: every note counts.
    assert_eq!(lay(&[window(0.0, 0.0, 0.0, 0.0, &[2.0, 100.0])]), [2.0, 100.0]);
}

// bakeAutomations' noteOns: looped, each repeat `k * loop < span`, a note outside the window is skipped.
#[test]
fn a_looped_clip_repeats_its_window_until_the_clip_ends() {
    // Window [1, 3): the note at 1 repeats every 2 beats in a clip 8 long; 0.5 and 3 are outside.
    assert_eq!(
        lay(&[window(10.0, 8.0, 1.0, 3.0, &[0.5, 1.0, 2.0, 3.0])]),
        [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0]
    );
}

// bakeAutomations' noteOns: a clip that ends before its loop does cuts the repeats at `at >= span`.
#[test]
fn a_clip_ending_before_its_loop_cuts_the_repeats() {
    assert_eq!(lay(&[window(0.0, 1.5, 0.0, 4.0, &[1.0, 2.0])]), [1.0]);
}

// bakeAutomations' noteOns: every beat is rounded, then sorted and made unique across the clips.
#[test]
fn the_notes_are_rounded_sorted_and_made_unique() {
    let a = window(0.0, 0.0, 0.0, 0.0, &[2.0, 1.0000001]);
    let b = window(0.0, 0.0, 0.0, 0.0, &[1.0, 2.0000004, 0.5]);
    assert_eq!(lay(&[a, b]), [0.5, 1.0, 2.0]);
    assert!(lay(&[]).is_empty());
}

fn baker_over(map: &IndexMap<String, Arc<CurveFile>>) -> CurveBaker<'_> {
    CurveBaker::new(map, sig(4, 4), 120.0, 16.0)
}

// bakeAutomations: `if (!curve) continue`, so a binding to no such curve is None and no finding.
#[test]
fn a_binding_to_no_such_curve_is_none_without_a_finding() {
    let map = curves(&[("a", "type(\"lfo\")\n")]);
    let mut baker = baker_over(&map);
    assert!(baker.bake("nope", &[]).is_none());
    let (findings, _) = baker.finish();
    assert!(findings.is_empty());
}

// decision 212: a curve that exists but bakes no point is None; an envelope with no events, a condition that never runs.
#[test]
fn a_curve_that_bakes_no_point_is_none() {
    let map = curves(&[
        ("env", "type(\"adsr\")\nattack(100)\n"),
        (
            "never",
            "type(\"lfo\")\nconditions([type(\"event\").event(\"Note On\")])\n",
        ),
        (
            "off",
            "type(\"lfo\")\nconditions([type(\"event\").event(\"Note Off\")])\n",
        ),
    ]);
    let mut baker = baker_over(&map);
    assert!(baker.bake("env", &[]).is_none());
    assert!(baker.bake("never", &[]).is_none());
    assert!(baker.bake("off", &[4.0]).is_none());
    assert!(baker.bake("env", &[4.0]).is_some());
    assert!(baker.bake("never", &[4.0]).is_some());
    assert!(baker.finish().0.is_empty());
}

// bakeAutomations: an envelope goes through envelope_fires then bake_adsr, every other curve through bake_curve.
#[test]
fn an_envelope_is_placed_by_its_events_and_the_others_by_song_time() {
    let map = curves(&[
        (
            "env",
            "type(\"adsr\")\nattack(100)\ndecay(100)\nsustain(200)\nrelease(100)\n",
        ),
        (
            "ramp",
            "type(\"custom\")\npoints([time(\"1:1:1\").value(0), time(\"3:1:1\").value(1)])\n",
        ),
    ]);
    let mut baker = baker_over(&map);
    let env = baker.bake("env", &[4.0]).expect("baked");
    assert_eq!(
        pairs(&env),
        [(4.0, 0.0), (4.2, 1.0), (4.4, 0.5), (4.8, 0.5), (5.0, 0.0)]
    );
    let ramp = baker.bake("ramp", &[4.0]).expect("baked");
    assert_eq!(pairs(&ramp), [(0.0, 0.0), (8.0, 1.0)]);
}

// decision 207: a curve is resolved once per baker, a refusal too, and the answer does not depend on the order asked.
#[test]
fn a_curve_is_resolved_once_and_a_refusal_is_reported_once() {
    let huge = "9".repeat(5000);
    let bad = format!("type(\"lfo\")\nconditions([type(\"start\").at(\"{huge}:1:1\")])\n");
    let map = curves(&[
        ("bad", bad.as_str()),
        ("good", "type(\"custom\")\npoints([time(\"1:1\").value(1)])\n"),
    ]);
    let mut baker = baker_over(&map);
    for _ in 0..1000 {
        assert!(baker.bake("bad", &[]).is_none());
        assert!(baker.bake("good", &[]).is_some());
    }
    let (findings, spent) = baker.finish();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "curves/bad.k7x");
    assert_eq!((findings[0].line, findings[0].col), (1, 1));
    assert!(spent > 0);
    let mut other = baker_over(&map);
    assert!(other.bake("good", &[]).is_some() && other.bake("bad", &[]).is_none());
}

// decision 208: a position or a duration too large to be a beat count is an ERROR named for the file, and bakes no lane.
#[test]
fn a_timecode_that_is_no_finite_beat_is_an_error_for_that_curve() {
    let huge = "9".repeat(5000);
    let duration = format!("type(\"lfo\")\nduration(\"{huge}\")\n");
    let map = curves(&[("long", duration.as_str()), ("ok", "type(\"lfo\")\n")]);
    let mut baker = baker_over(&map);
    assert!(baker.bake("long", &[]).is_none());
    assert!(baker.bake("ok", &[]).is_some());
    let (findings, _) = baker.finish();
    assert_eq!(
        findings.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(),
        ["curves/long.k7x"]
    );
    assert!(findings[0].message.len() < 200);
}

// decision 208: a breakpoint that is not finite (an enormous stage at a huge tempo) is an ERROR, and that lane is dropped.
#[test]
fn a_breakpoint_that_is_not_finite_is_an_error() {
    let map = curves(&[("env", "type(\"adsr\")\nattack(1e308)\n")]);
    let mut baker = CurveBaker::new(&map, sig(4, 4), 1e300, 16.0);
    assert!(baker.bake("env", &[1.0]).is_none());
    let (findings, _) = baker.finish();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "curves/env.k7x");
}

// decision 211: past the budget the baker is capped, one ERROR names the curve it stopped in, and it answers None from then on.
#[test]
fn past_the_budget_the_baker_is_capped_for_good() {
    let map = curves(&[("env", "type(\"adsr\")\nattack(100)\n"), ("a", "type(\"lfo\")\n")]);
    let mut baker = baker_over(&map);
    let notes: Vec<f64> = (0..3_000_000).map(|i| f64::from(i) * 2.0).collect();
    assert!(baker.bake("env", &notes).is_none());
    assert!(baker.bake("a", &[]).is_none());
    assert!(baker.note_ons(&[]).is_empty());
    let (findings, spent) = baker.finish();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "curves/env.k7x");
    assert!(spent > 1_000_000);
}

// decision 201: a file that did not decode is not in what `read_curves` answers, so the baker has no such curve.
#[test]
fn a_file_that_did_not_decode_is_no_curve() {
    let src = support::mem(&[
        ("curves/nope.k7x", "type(\"nope\")\n"),
        ("curves/ok.k7x", "type(\"lfo\")\n"),
    ]);
    let read = read_curves(&src, &mut Cache::new(), "/p").expect("reads");
    assert_eq!(read.curves.keys().collect::<Vec<_>>(), ["ok"]);
    assert_eq!(read.findings.len(), 1);
    let mut baker = baker_over(&read.curves);
    assert!(baker.bake("nope", &[]).is_none());
    assert!(baker.bake("ok", &[]).is_some());
    assert!(baker.finish().0.is_empty());
}
