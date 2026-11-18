// curves_conditions.rs — WHEN a curve runs: the runs, the firings, and what the bake writes for them; from curveConditions.test.ts.
// A reader would get wrong that a bipolar on/off or a cut is not special here: conditions and cut are separate passes.

use crate::curves_support;

use curves_support::{adsr, bake44, budget, fires, fires_under, pairs, r44};
use song::curves::{CurveRun, EnvelopeFire, bake_adsr, curve_runs, is_note_on_event};

fn runs(text: &str, end: f64, triggers: &[f64]) -> Vec<CurveRun> {
    curve_runs(&r44(text), end, triggers, &mut budget()).expect("runs")
}

fn run(from: f64, to: f64, origin: f64) -> CurveRun {
    CurveRun { from, to, origin }
}

fn lfo(conditions: &str) -> String {
    format!("type(\"lfo\")\nconditions([{conditions}])\n")
}

// curveConditions.test.ts "runs a curve from a moment, carrying on its song time unless retriggered".
#[test]
fn a_start_runs_from_a_moment_carrying_on_unless_retriggered() {
    assert_eq!(
        runs(&lfo("type(\"start\").at(\"3:1:1\")"), 32.0, &[]),
        [run(8.0, 32.0, 0.0)]
    );
    assert_eq!(
        runs(&lfo("type(\"start\").at(\"3:1:1\").retrigger(true)"), 32.0, &[]),
        [run(8.0, 32.0, 8.0)]
    );
}

// curveConditions.test.ts "ends one at a moment, and runs one between two".
#[test]
fn an_end_and_a_between_are_their_stretches() {
    assert_eq!(
        runs(&lfo("type(\"end\").at(\"3:1:1\")"), 32.0, &[]),
        [run(0.0, 8.0, 0.0)]
    );
    let between = "type(\"between\").start(\"3:1:1\").end(\"5:1:1\").retrigger(true)";
    assert_eq!(runs(&lfo(between), 32.0, &[]), [run(8.0, 16.0, 8.0)]);
    // A between is not limited by the song's end (decision 209), and an empty one never runs.
    assert_eq!(
        runs(&lfo("type(\"between\").start(\"3:1:1\").end(\"20:1:1\")"), 8.0, &[]),
        [run(8.0, 76.0, 0.0)]
    );
    assert!(runs(&lfo("type(\"between\").start(\"5:1:1\").end(\"3:1:1\")"), 32.0, &[]).is_empty());
    assert!(runs(&lfo("type(\"start\").at(\"9:1:1\")"), 8.0, &[]).is_empty());
}

// curveConditions.test.ts "starts one at its node's first note, or restarts it at every note when retriggered".
#[test]
fn an_event_starts_at_the_first_note_or_restarts_at_every_one() {
    let notes = [12.0, 4.0];
    assert_eq!(
        runs(&lfo("type(\"event\").event(\"Note On\")"), 32.0, &notes),
        [run(4.0, 32.0, 4.0)]
    );
    assert_eq!(
        runs(&lfo("type(\"event\").event(\"noteOn\").retrigger(true)"), 32.0, &notes),
        [run(4.0, 12.0, 4.0), run(12.0, 32.0, 12.0)]
    );
}

// curveConditions.test.ts "never runs on an event its node does not announce".
#[test]
fn an_event_the_node_does_not_announce_never_runs() {
    assert!(is_note_on_event("note-on") && is_note_on_event("Note On") && is_note_on_event("NOTEON"));
    assert!(!is_note_on_event("Note Off") && !is_note_on_event("") && !is_note_on_event("noteonn"));
    assert!(runs(&lfo("type(\"event\").event(\"Note Off\")"), 32.0, &[4.0]).is_empty());
    assert!(runs(&lfo("type(\"event\").event(\"Note On\")"), 32.0, &[]).is_empty());
}

// curveConditions.test.ts "run the curve while ANY holds: two windows are two stretches".
#[test]
fn several_conditions_run_the_curve_while_any_holds() {
    let c = lfo("type(\"between\").start(\"2:1:1\").end(\"3:1:1\"), type(\"start\").at(\"5:1:1\")");
    assert_eq!(runs(&c, 32.0, &[]), [run(4.0, 8.0, 0.0), run(16.0, 32.0, 0.0)]);
}

// curveConditions.test.ts "widen one stretch where they overlap, and restart it where a retriggering one comes true".
#[test]
fn overlapping_conditions_widen_and_a_retriggering_one_restarts() {
    let end = "type(\"end\").at(\"4:1:1\")";
    let plain = lfo(&format!("{end}, type(\"start\").at(\"3:1:1\")"));
    assert_eq!(runs(&plain, 32.0, &[]), [run(0.0, 32.0, 0.0)]);
    let again = lfo(&format!("{end}, type(\"start\").at(\"3:1:1\").retrigger(true)"));
    assert_eq!(runs(&again, 32.0, &[]), [run(0.0, 8.0, 0.0), run(8.0, 32.0, 8.0)]);
}

// curveConditions.test.ts "fire an envelope on any condition's notes, each cut at the end of its own window".
#[test]
fn an_envelope_fires_on_any_conditions_notes_cut_at_its_own_window() {
    let c = r44(&lfo(
        "type(\"between\").start(\"1:1:1\").end(\"2:1:1\"), type(\"between\").start(\"4:1:1\").end(\"5:1:1\")",
    ));
    assert_eq!(
        fires_under(&c, &[1.0, 6.0, 13.0]),
        [
            EnvelopeFire {
                at: 1.0,
                stop: Some(4.0),
                retrigger: false
            },
            EnvelopeFire {
                at: 13.0,
                stop: Some(16.0),
                retrigger: false
            },
        ]
    );
}

// decision 209: two conditions firing at one beat are one firing, the longer window, retriggering if either does.
#[test]
fn two_firings_at_one_beat_merge_into_the_longer_window() {
    let c = r44(&lfo(
        "type(\"between\").start(\"1:1:1\").end(\"2:1:1\"), type(\"between\").start(\"1:1:1\").end(\"3:1:1\").retrigger(true)",
    ));
    let got = fires_under(&c, &[0.0, 1.0]);
    assert_eq!(
        got,
        [
            EnvelopeFire {
                at: 0.0,
                stop: Some(8.0),
                retrigger: true
            },
            EnvelopeFire {
                at: 1.0,
                stop: Some(8.0),
                retrigger: true
            }
        ]
    );
    let open = r44(&lfo("type(\"event\").event(\"noteOn\"), type(\"end\").at(\"2:1:1\")"));
    assert_eq!(
        fires_under(&open, &[1.0]),
        [EnvelopeFire {
            at: 1.0,
            stop: None,
            retrigger: false
        }]
    );
    // A retriggering start fires at the window's opening too.
    let opening = r44(&lfo("type(\"start\").at(\"2:1:1\").retrigger(true)"));
    let at: Vec<f64> = fires_under(&opening, &[1.0, 5.0]).iter().map(|f| f.at).collect();
    assert_eq!(at, [4.0, 5.0]);
}

// curveConditions.test.ts "writes nothing before a start: the engine holds the first point, the curve's value there".
#[test]
fn nothing_is_written_before_a_start() {
    let saw = "type(\"lfo\")\nshape(\"saw\")\nconditions([type(\"start\").at(\"3:1:1\").retrigger(true)])\n";
    let b = bake44(saw, 32.0, &[]);
    assert_eq!(b.points[0], [8.0, 0.0]);
    assert!(b.points.iter().all(|p| p[0] >= 8.0));
}

// curveConditions.test.ts "restarts at each retriggering event, the jump a step rather than a slope".
#[test]
fn a_retriggering_event_restarts_the_curve_as_a_step() {
    let saw = "type(\"lfo\")\nshape(\"saw\")\nconditions([type(\"event\").event(\"Note On\").retrigger(true)])\n";
    let b = bake44(saw, 32.0, &[4.0, 6.0]);
    let at6: Vec<f64> = b
        .points
        .iter()
        .filter(|p| (p[0] - 6.0).abs() <= 1e-3)
        .map(|p| p[1])
        .collect();
    assert_eq!(at6, [0.5, 0.0]);
}

const ENV: &str = "type(\"adsr\")\nattack(1000)\ndecay(0)\nsustain(0)\nrelease(0)\nconditions([type(\"between\").start(\"2:1:1\").end(\"3:1:1\")])\n";

// curveConditions.test.ts "fires an envelope only inside its window, cut where the window ends".
#[test]
fn an_envelope_fires_only_inside_its_window_and_is_cut_where_it_ends() {
    let c = r44(ENV);
    let fired = fires_under(&c, &[0.0, 5.0, 10.0]);
    assert_eq!(
        fired,
        [EnvelopeFire {
            at: 5.0,
            stop: Some(8.0),
            retrigger: false
        }]
    );
    let b = bake_adsr(&c, &fired, 120.0, &mut budget()).expect("bakes");
    assert!(b.points[b.points.len() - 1][0] < 8.0);
}

// curveConditions.test.ts "ignores a note landing while an envelope runs, unless retriggered".
#[test]
fn a_note_landing_inside_a_running_envelope_is_ignored_unless_retriggered() {
    let c = r44("type(\"adsr\")\nattack(1000)\nconditions([type(\"event\").event(\"Note On\")])\n");
    let zeros = |retrigger: bool| {
        let b = bake_adsr(&c, &fires(&[0.0, 1.0], retrigger), 120.0, &mut budget()).expect("bakes");
        b.points.iter().filter(|p| p[1] == 0.0).count()
    };
    assert!(zeros(false) < zeros(true));
}

const SWITCH: &str =
    "type(\"onoff\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1), time(\"3:1:1\").value(0)])\n";

// curveConditions.test.ts "bakes as STEPS: each change held until the microbeat before the next".
#[test]
fn an_on_off_curve_bakes_as_steps() {
    assert_eq!(
        pairs(&bake44(SWITCH, 16.0, &[])),
        [(0.0, 0.0), (3.999999, 0.0), (4.0, 1.0), (7.999999, 1.0), (8.0, 0.0)]
    );
}

// curveConditions.test.ts "steps inside a condition's window too".
#[test]
fn an_on_off_curve_steps_inside_a_conditions_window() {
    let later = "type(\"onoff\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1)])\nconditions([type(\"start\").at(\"3:1:1\").retrigger(true)])\n";
    let b = bake44(later, 32.0, &[]);
    assert!(b.points.iter().all(|p| p[1] == 0.0 || p[1] == 1.0));
    assert_eq!(b.points.iter().find(|p| p[0] >= 12.0).map(|p| p[1]), Some(1.0));
}

// curveConditions.test.ts "keeps only its positive half, with a breakpoint where it crosses 0".
#[test]
fn a_positive_cut_adds_the_crossing_point() {
    let tri = "type(\"custom\")\npolarity(\"bipolar\")\ncut(\"positive\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1)])\n";
    assert_eq!(pairs(&bake44(tri, 4.0, &[])), [(0.0, 0.0), (2.0, 0.0), (4.0, 1.0)]);
}

// curveConditions.test.ts "keeps only its negative half".
#[test]
fn a_negative_cut_keeps_only_the_negative_half() {
    let sine = "type(\"lfo\")\npolarity(\"bipolar\")\ncut(\"negative\")\n";
    let b = bake44(sine, 16.0, &[]);
    assert!(b.points.iter().all(|p| p[1] <= 0.0) && b.points.iter().any(|p| p[1] < 0.0));
}

// curveConditions.test.ts "cuts an envelope too".
#[test]
fn a_cut_envelope_keeps_only_its_positive_half() {
    let env = "type(\"adsr\")\npolarity(\"bipolar\")\ncut(\"positive\")\nattack(1000)\n";
    assert!(adsr(env, &[0.0], 120.0).points.iter().all(|p| p[1] >= 0.0));
}

// curveConditions.test.ts "runs that long from its start and writes nothing after, the engine holding where it stopped".
#[test]
fn a_duration_ends_the_run_that_long_after_its_start() {
    let b = bake44("type(\"lfo\")\nshape(\"saw\")\nduration(\"1:0\")\n", 32.0, &[]);
    assert_eq!(b.points[0][0], 0.0);
    assert_eq!(b.points[b.points.len() - 1][0], 4.0);
}

// curveConditions.test.ts "restarts with each retriggering run, lasting the same each time".
#[test]
fn a_duration_restarts_with_each_retriggering_run() {
    let lfo = "type(\"lfo\")\nduration(\"0:2\")\nconditions([type(\"event\").event(\"Note On\").retrigger(true)])\n";
    let beats: Vec<f64> = bake44(lfo, 32.0, &[4.0, 12.0]).points.iter().map(|p| p[0]).collect();
    assert!(!beats.iter().any(|b| *b > 6.0 && *b < 12.0));
    assert!(!beats.iter().any(|b| *b > 14.0));
    assert!(beats.contains(&12.0));
}
