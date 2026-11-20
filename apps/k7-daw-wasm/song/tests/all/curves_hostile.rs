// curves_hostile.rs — hostile curve files and bakes, each refused fast: the answer is asserted and so is the time (decision 211).
// A reader would get wrong that the limit counts the OUTPUT: a small file can name millions of breakpoints.

use crate::curves_support;

use curves_support::{budget, decode, file, fires, pairs, r44, resolved, sig};
use indexmap::IndexMap;
use song::curves::{BUDGET, CurveBaker, CurveFile, MidiWindow, bake_adsr};
use song::{Finding, Severity};
use std::sync::Arc;
use std::time::{Duration, Instant};

const HANG_BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = f();
    assert!(started.elapsed() < HANG_BOUND, "{what} took {:?}", started.elapsed());
    out
}

fn map(files: &[(&str, String)]) -> IndexMap<String, Arc<CurveFile>> {
    files
        .iter()
        .map(|(id, text)| ((*id).to_owned(), Arc::new(file(text))))
        .collect()
}

fn baker(m: &IndexMap<String, Arc<CurveFile>>) -> CurveBaker<'_> {
    CurveBaker::new(m, sig(4, 4), 120.0, 16.0)
}

fn only_finding(findings: Vec<Finding>, name: &str) {
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].file, name);
    assert_eq!(
        (findings[0].line, findings[0].col, findings[0].severity),
        (1, 1, Severity::Error)
    );
}

// decision 202: each hostile row of the decoders is a finding, fast.
#[test]
fn every_hostile_curve_file_is_refused_fast() {
    let nest = format!("type(\"custom\")\npoints({}{})\n", "[".repeat(300), "]".repeat(300));
    let rows: Vec<String> = [
        "",
        "shape(\"saw\")\n",
        "type(1)\n",
        "type(\"x\")\n",
        "type(\"lfo\")\nfrequency(0)\n",
        "type(\"lfo\")\nfrequency(-1)\n",
        "type(\"lfo\")\nfrequency(1e400)\n",
        "type(\"lfo\")\namplitude(1.5)\n",
        "type(\"lfo\")\namplitude(-0.1)\n",
        "type(\"lfo\")\nphase(2)\n",
        "type(\"adsr\")\nlevel(2)\n",
        "type(\"adsr\")\nattack(-1)\n",
        "type(\"adsr\")\nattack(1e400)\n",
        "type(\"lfo\")\npolarity(\"x\")\n",
        "type(\"lfo\")\nshape(\"x\")\n",
        "type(\"lfo\")\ncut(\"x\")\n",
        "type(\"lfo\")\nz(1)\n",
        "type(\"adsr\")\nz(1)\n",
        "type(\"custom\")\nz(1)\npoints([time(\"1:1\").value(1)])\n",
        "type(\"onoff\")\nz(1)\npoints([time(\"1:1\").value(1)])\n",
        "type(\"onoff\")\npolarity(\"bipolar\")\npoints([time(\"1:1\").value(1)])\n",
        "type(\"onoff\")\ncut(\"positive\")\npoints([time(\"1:1\").value(1)])\n",
        "type(\"onoff\")\namplitude(0.5)\npoints([time(\"1:1\").value(1)])\n",
        "type(\"onoff\")\npoints([time(\"1:1\").value(0.5)])\n",
        "type(\"custom\")\npoints([time(\"1:1\").value(\"x\")])\n",
        "type(\"custom\")\npoints([])\n",
        "type(\"custom\")\npoints(1)\n",
        "type(\"custom\")\npoints([value(1)])\n",
        "type(\"custom\")\npoints([time(\"1:1\").value(1).z(1)])\n",
        "type(\"custom\")\npoints([time(1).value(1)])\n",
        "type(\"custom\")\npoints([time(\"x\").value(1)])\n",
        "type(\"lfo\")\nduration(\"x\")\n",
        "type(\"lfo\")\nconditions(1)\n",
        "type(\"lfo\")\nconditions([1])\n",
        "type(\"lfo\")\nconditions([{ at: \"1:1\" }])\n",
        "type(\"lfo\")\nconditions([type(\"z\")])\n",
        "type(\"lfo\")\nconditions([type(\"start\").at(\"1:1\").z(1)])\n",
        "type(\"lfo\")\nconditions([type(\"start\").at(\"x\")])\n",
        "type(\"lfo\")\nconditions([type(\"event\").event(\"\")])\n",
        "type(\"lfo\")\nconditions([type(\"start\").at(\"1:1\").retrigger(\"yes\")])\n",
        "type(\"lfo\")\nconditions([type(\"between\").start(\"1:1\")])\n",
        nest.as_str(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    timed("the decoder rows", || {
        for row in &rows {
            assert!(decode(row).is_err(), "{row:?} should be refused");
        }
    });
}

// decision 224: a word of 5,000 characters in a refused file is cut short in its WARNING, wherever it sits.
#[test]
fn a_long_word_in_a_refused_file_is_cut_short_in_its_message() {
    let w = "A".repeat(5000);
    let nines = "9".repeat(5000);
    let rows = [
        format!("type(\"lfo\")\nshape(\"{w}\")\n"),
        format!("type(\"{w}\")\n"),
        format!("type(\"lfo\")\n{w}(1)\n"),
        format!("type(\"lfo\")\nfrequency(\"{nines}\")\n"),
        format!("type(\"lfo\")\ncut(\"{nines}\")\n"),
        format!("type(\"lfo\")\nconditions([type(\"{w}\")])\n"),
        format!("type(\"lfo\")\nconditions([type(\"start\").at(\"1:1\").{w}(1)])\n"),
    ];
    for row in &rows {
        let f = decode(row).expect_err("refused");
        assert!(f.message.len() < 300, "{} characters", f.message.len());
        assert!(f.message.len() > 10, "{}", f.message);
    }
}

// decision 211: a million points in one file is read, fast, and is the file it says.
#[test]
fn a_million_points_in_one_file_are_read_in_time() {
    let mut text = String::from("type(\"custom\")\npoints([\n");
    for i in 0..1_000_000 {
        text.push_str(&format!("time(\"1:1:1.{i}\").value(0.5),\n"));
    }
    text.push_str("])\n");
    let f = timed("a million points", || decode(&text).expect("decodes"));
    assert!(matches!(&f.spec, song_schema::CurveSpec::Custom(p) if p.points.len() == 1_000_000));
}

// decision 208: a position or a duration of 5,000 digits does not resolve; a chain of 40 fields and a 1/4096 signature do.
#[test]
fn a_huge_timecode_does_not_resolve_and_a_long_chain_or_a_deep_signature_does() {
    let huge = "7".repeat(5000);
    let position = file(&format!(
        "type(\"lfo\")\nconditions([type(\"start\").at(\"{huge}\")])\n"
    ));
    let duration = file(&format!("type(\"lfo\")\nduration(\"{huge}\")\n"));
    timed("the huge timecodes", || {
        assert!(song::curves::ResolvedCurve::new(&position, sig(4, 4)).is_err());
        assert!(song::curves::ResolvedCurve::new(&duration, sig(4, 4)).is_err());
    });
    let chain = vec!["1"; 40].join(":");
    let c = resolved(
        &format!("type(\"lfo\")\nconditions([type(\"start\").at(\"{chain}\")])\n"),
        sig(4, 4),
    );
    assert_eq!(c.conditions().len(), 1);
    assert!(resolved("type(\"lfo\")\n", sig(1, 4096)).curve().sig().bar() < 0.001);
    let m = IndexMap::from([("a".to_owned(), Arc::new(file("type(\"lfo\")\n")))]);
    let mut b = CurveBaker::new(&m, sig(1, 4096), 120.0, 100_000.0);
    let baked = timed("a 1/4096 bar", || b.bake("a", &[])).expect("baked");
    assert!(baked.points.len() >= 2048 && baked.points.len() <= 2050);
}

// decision 211: 200,000 points under 20,000 retriggering events write far more than the budget, and it stops there.
#[test]
fn many_points_under_many_retriggers_stop_at_the_budget() {
    let mut text = String::from("type(\"custom\")\npoints([\n");
    for i in 0..200_000 {
        text.push_str(&format!("time(\"1:1:1.{i}\").value(0.5),\n"));
    }
    text.push_str("])\nconditions([type(\"event\").event(\"noteOn\").retrigger(true)])\n");
    let m = map(&[("a", text)]);
    let triggers: Vec<f64> = (0..20_000).map(|i| f64::from(i) * 10.0).collect();
    let mut b = baker(&m);
    assert!(timed("the points under the retriggers", || b.bake("a", &triggers)).is_none());
    let (findings, spent) = b.finish();
    only_finding(findings, "curves/a.k7x");
    assert!(spent <= BUDGET);
}

// decision 211: an oscillator under a 1e300 song is capped at 2,049 points.
#[test]
fn an_oscillator_under_a_huge_song_is_capped_by_its_grid() {
    let m = map(&[("a", "type(\"lfo\")\n".to_owned())]);
    let mut b = CurveBaker::new(&m, sig(4, 4), 120.0, 1e300);
    let baked = timed("a 1e300 song", || b.bake("a", &[])).expect("baked");
    assert!(
        baked.points.len() == 2049 || baked.points.len() == 2050,
        "{}",
        baked.points.len()
    );
    assert!(b.finish().0.is_empty());
}

// decision 211: a `between` so far out that `b += step` stops moving ends in the budget, not in a loop.
#[test]
fn a_between_far_enough_out_to_stop_the_grid_ends_in_the_budget() {
    let text = "type(\"lfo\")\nconditions([type(\"between\").start(\"25000000000000001:1:1\").end(\"25000000000000005:1:1\")])\n".to_owned();
    let m = map(&[("a", text)]);
    let mut b = baker(&m);
    assert!(timed("the stuck grid", || b.bake("a", &[])).is_none());
    only_finding(b.finish().0, "curves/a.k7x");
}

// decision 211, 221: 100,000 conditions are read and baked; 100,000 retriggering ones by a million note-ons are past the budget.
#[test]
fn a_hundred_thousand_conditions_bake_and_retriggering_ones_by_a_million_note_ons_stop_at_the_budget() {
    let starts: Vec<String> = (1..=100_000)
        .map(|i| format!("type(\"start\").at(\"{}:1:1\")", i % 5000 + 1))
        .collect();
    let m = map(&[("a", format!("type(\"lfo\")\nconditions([{}])\n", starts.join(", ")))]);
    let mut b = baker(&m);
    let baked = timed("100,000 start conditions", || b.bake("a", &[]));
    assert!(baked.is_some_and(|c| !c.points.is_empty()));
    assert!(b.finish().0.is_empty());
    let events: Vec<String> = (0..100_000)
        .map(|_| "type(\"event\").event(\"noteOn\").retrigger(true)".to_owned())
        .collect();
    let m = map(&[("e", format!("type(\"lfo\")\nconditions([{}])\n", events.join(", ")))]);
    let notes: Vec<f64> = (0..1_000_000).map(f64::from).collect();
    let mut b = baker(&m);
    assert!(timed("100,000 conditions by a million notes", || b.bake("e", &notes)).is_none());
    only_finding(b.finish().0, "curves/e.k7x");
}

// decision 211: an envelope on three million note-ons is past the budget.
#[test]
fn an_envelope_on_three_million_note_ons_stops_at_the_budget() {
    let m = map(&[(
        "e",
        "type(\"adsr\")\nattack(100)\ndecay(100)\nsustain(100)\nrelease(100)\n".to_owned(),
    )]);
    let notes: Vec<f64> = (0..3_000_000).map(|i| f64::from(i) * 2.0).collect();
    let mut b = baker(&m);
    assert!(timed("three million firings", || b.bake("e", &notes)).is_none());
    only_finding(b.finish().0, "curves/e.k7x");
}

// decision 209: an envelope with every stage 0 on 100,000 note-ons at one beat writes a few points each, all finite.
#[test]
fn an_envelope_of_zero_stages_on_one_beat_is_finite_and_bounded() {
    let c = r44("type(\"adsr\")\n");
    let at_one_beat = fires(&vec![1.0; 100_000], true);
    let baked = timed("zero stages", || bake_adsr(&c, &at_one_beat, 120.0, &mut budget())).expect("within the budget");
    assert!(!baked.points.is_empty() && baked.points.len() <= 400_000);
    assert!(baked.points.iter().flatten().all(|v| v.is_finite()));
    assert_eq!(pairs(&baked)[0], (1.0, 0.0));
}

// decision 208: `attack: 1e308` at tempo 1e300 bakes a breakpoint that is not finite, an ERROR.
#[test]
fn an_enormous_stage_at_a_huge_tempo_is_an_error() {
    let m = map(&[("e", "type(\"adsr\")\nattack(1e308)\n".to_owned())]);
    let mut b = CurveBaker::new(&m, sig(4, 4), 1e300, 16.0);
    assert!(timed("1e308 at 1e300", || b.bake("e", &[1.0])).is_none());
    only_finding(b.finish().0, "curves/e.k7x");
}

// decision 207: one curve bound 100,000 times is resolved once and bakes every lane.
#[test]
fn one_curve_bound_a_hundred_thousand_times_is_resolved_once() {
    let m = map(&[(
        "a",
        "type(\"custom\")\npoints([time(\"1:1\").value(0), time(\"3:1\").value(1)])\n".to_owned(),
    )]);
    let mut b = baker(&m);
    timed("100,000 bindings", || {
        for _ in 0..100_000 {
            assert!(b.bake("a", &[]).is_some());
        }
    });
    assert!(b.finish().0.is_empty());
}

fn note_ons_of(w: &[MidiWindow<'_>]) -> (Vec<f64>, Vec<Finding>) {
    let m = IndexMap::new();
    let mut b = CurveBaker::new(&m, sig(4, 4), 120.0, 16.0);
    let out = timed("note_ons", || b.note_ons(w));
    (out, b.finish().0)
}

// decision 211: a loop of 1e-9 in a length of 1e6, and a loop in a length of 0, never end: one ERROR named `curves`.
#[test]
fn a_loop_that_would_never_end_is_capped() {
    let notes = [0.0];
    for (length, loop_end) in [(1e6, 1e-9), (0.0, 1.0)] {
        let w = MidiWindow {
            start: 0.0,
            length,
            loop_start: 0.0,
            loop_end,
            note_times: &notes,
        };
        let (out, findings) = note_ons_of(&[w]);
        assert!(out.is_empty());
        only_finding(findings, "curves");
    }
}

// decision 214: two million notes are within the budget, sorted and unique.
#[test]
fn two_million_notes_are_laid_within_the_budget() {
    let notes: Vec<f64> = (0..2_000_000).map(|i| f64::from(i) / 8.0).collect();
    let w = MidiWindow {
        start: 0.0,
        length: 0.0,
        loop_start: 0.0,
        loop_end: 0.0,
        note_times: &notes,
    };
    let (out, findings) = note_ons_of(&[w]);
    assert_eq!(out.len(), 2_000_000);
    assert!(findings.is_empty());
    assert!(out.windows(2).all(|p| p[0] < p[1]));
}

// decision 221: windows whose notes all lie outside their loop lay nothing and still pay for the walk.
#[test]
fn windows_that_lay_nothing_still_pay_for_their_walk() {
    let notes: Vec<f64> = (0..1_000_000).map(|i| 100.0 + f64::from(i)).collect();
    let windows: Vec<MidiWindow> = (0..3000)
        .map(|_| MidiWindow {
            start: 0.0,
            length: 0.0,
            loop_start: 0.0,
            loop_end: 4.0,
            note_times: &notes,
        })
        .collect();
    let (out, findings) = note_ons_of(&windows);
    assert!(out.is_empty());
    only_finding(findings, "curves");
}

// decision 221: an event that is not a note-on walks nothing, so 101 of them on 40,000 note-ons are no lane and no ERROR.
#[test]
fn an_event_that_is_not_a_note_on_costs_nothing() {
    let events: Vec<String> = (0..101)
        .map(|_| "type(\"event\").event(\"Pitch\")".to_owned())
        .collect();
    let m = map(&[("e", format!("type(\"lfo\")\nconditions([{}])\n", events.join(", ")))]);
    let notes: Vec<f64> = (0..40_000).map(f64::from).collect();
    let mut b = baker(&m);
    assert!(timed("101 pitch events", || b.bake("e", &notes)).is_none());
    let (findings, spent) = b.finish();
    assert!(findings.is_empty(), "{findings:?}");
    assert!(spent < 1000, "{spent}");
}

// decision 211: a dozen lanes of an hour of sixteenths, two cut envelopes on each, bake within one baker's budget.
#[test]
fn a_dozen_full_size_lanes_with_two_envelopes_each_stay_within_the_budget() {
    let env = "type(\"adsr\")\nattack(50)\ndecay(50)\nsustain(50)\nrelease(50)\ncut(\"positive\")\n";
    let m = map(&[("a", env.to_owned()), ("b", env.replace("attack(50)", "attack(60)"))]);
    let notes: Vec<f64> = (0..28_800).map(|i| f64::from(i) / 4.0).collect();
    let mut b = baker(&m);
    timed("twelve full-size lanes", || {
        for _ in 0..12 {
            let w = MidiWindow {
                start: 0.0,
                length: 0.0,
                loop_start: 0.0,
                loop_end: 0.0,
                note_times: &notes,
            };
            let events = b.note_ons(&[w]);
            for id in ["a", "b"] {
                assert!(b.bake(id, &events).is_some_and(|c| c.points.len() > 100_000), "{id}");
            }
        }
    });
    let (findings, spent) = b.finish();
    assert!(findings.is_empty(), "{findings:?}");
    assert!(spent < BUDGET, "{spent}");
}
