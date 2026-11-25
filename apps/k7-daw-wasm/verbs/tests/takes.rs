// takes.rs — an event's takes on a real sequence lane: a MIDI pass filed as a take, comped, faded at a seam, dropped; arming asked of the host.
// A reader would get wrong the fixture: edit-undo has no takes, so the first test files one with `event take midi` and works on it.

mod support;

use serde_json::json;
use support::Project;

const PASS: &str =
    r#"{"start":0,"end":4,"notes":[{"t":0,"d":1,"k":62,"v":0.7},{"t":2,"d":1,"k":65},{"t":3,"d":0,"k":1}]}"#;
/// The take `with_take` leaves: the undone run took `id-1`, the one kept `id-2`.
const TAKE: &str = "melody_1.1_id-2";

fn with_take() -> Project {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip(&format!("event take midi melody 1:1:1 {PASS}"));
    assert_eq!(ran.answer.log, ["melody: take melody_1.1_id-1, 2 note(s)"]);
    assert_eq!(
        ran.changed(&p.dir),
        ["nodes/melody.k7x", "sequences/melody_1.1_id-1.k7x"]
    );
    p.ok(&format!("event take midi melody 1:1:1 {PASS}"));
    p
}

#[test]
fn a_midi_pass_is_a_sequence_take_comped_over_what_was_played() {
    let mut p = with_take();
    let lane = p.text("nodes/melody.k7x");
    assert!(
        lane.contains(&format!("ref(\"{TAKE}\")")) && lane.contains("comp("),
        "{lane}"
    );
    let seq = p.data(&format!("sequences/{TAKE}.k7x"));
    let notes = seq.get("notes").expect("notes");
    assert!(
        matches!(notes, k7x::edit::Value::List(l) if l.len() == 2),
        "the zero-length note is dropped"
    );
    let keys: Vec<f64> = p
        .track("melody")
        .clips
        .iter()
        .flat_map(|c| c.notes.iter().map(|n| f64::from(n.key)))
        .collect();
    assert!(
        keys.contains(&62.0) && !keys.contains(&60.0),
        "the take plays over the pass: {keys:?}"
    );
    p.refused("event take midi melody 1:1:1 {nope", "the pass is not JSON");
    p.refused(
        r#"event take midi melody 1:1:1 {"start":2,"end":1,"notes":[]}"#,
        "a start before its end",
    );
    p.refused(&format!("event take midi nobody 1:1:1 {PASS}"), "no track \"nobody\"");
}

#[test]
fn a_comp_picks_a_take_over_a_span_and_a_seam_takes_a_crossfade() {
    let mut p = with_take();
    let ran = p.round_trip("event comp melody 1:1:1 melody 1:3 2:1");
    assert_eq!(ran.answer.log, ["melody: melody plays 1:3 → 2:1"]);
    p.ok("event comp melody 1:1:1 melody 1:3 2:1");
    let keys: Vec<f64> = p
        .track("melody")
        .clips
        .iter()
        .flat_map(|c| c.notes.iter().map(|n| f64::from(n.key)))
        .collect();
    assert!(
        keys.contains(&62.0) && keys.contains(&67.0),
        "the take then the event's own: {keys:?}"
    );
    let ran = p.round_trip("event comp fade melody 1:1:1 1:3 0:0:1");
    assert_eq!(ran.answer.log, ["melody: crossfade of 0:0:1 at 1:3"]);
    p.ok("event comp fade melody 1:1:1 1:3 0:0:1");
    assert!(p.text("nodes/melody.k7x").contains("0:0:1"));
    let ran = p.ok("event comp fade melody 1:1:1 1:3 0");
    assert_eq!(ran.answer.log, ["melody: no crossfade at 1:3"]);
    assert!(!p.text("nodes/melody.k7x").contains("0:0:1"));
    p.refused("event comp fade melody 1:1:1 1:3 x", "<length> expects");
    p.refused("event comp melody 1:1:1 melody 1:3", "usage: event comp");
    p.refused("event comp melody 3:1:1 melody 1:3 2:1", "event comp failed");
}

#[test]
fn a_take_removed_leaves_its_material_and_its_spans_go() {
    let mut p = with_take();
    let ran = p.round_trip(&format!("event take remove melody 1:1:1 {TAKE}"));
    assert_eq!(ran.answer.log, [format!("melody: take {TAKE} removed")]);
    p.ok(&format!("event take remove melody 1:1:1 {TAKE}"));
    assert!(!p.text("nodes/melody.k7x").contains(TAKE));
    assert!(p.exists(&format!("sequences/{TAKE}.k7x")), "the material stays on disk");
    let keys: Vec<f64> = p
        .track("melody")
        .clips
        .iter()
        .flat_map(|c| c.notes.iter().map(|n| f64::from(n.key)))
        .collect();
    assert!(keys.contains(&60.0) && !keys.contains(&62.0), "{keys:?}");
    p.refused("event take remove melody 1:1:1 ghost", "event take remove failed");
}

#[test]
fn arming_is_asked_of_the_host_and_writes_nothing() {
    let mut p = Project::open("edit-undo");
    let ran = p.ok("event take arm melody 1:1:1 1:2 2:1");
    assert_eq!(ran.before, ran.after);
    assert!(ran.answer.effects.is_empty());
    assert_eq!(
        ran.answer.asks,
        [
            json!({ "member": "takeArm", "args": [{ "track": "melody", "eventBeats": 0.0, "punch": { "from": 1.0, "to": 4.0 } }] })
        ]
    );
    assert_eq!(
        ran.answer.log,
        ["melody: takes go into the event at 1:1:1, punched 1:2 → 2:1"]
    );
    let open = p.ok("event take arm melody 1:1:1 1:2");
    assert_eq!(open.answer.asks[0]["args"][0]["punch"], json!({ "from": 1.0 }));
    let off = p.ok("event take disarm");
    assert_eq!(off.answer.asks, [json!({ "member": "takeArm", "args": [null] })]);
    p.refused(
        "event take arm melody 1:1:1 2:1 1:2",
        "the punch must end after it starts",
    );
    p.refused("event take arm melody 1:1:1", "usage: event take arm");
}
