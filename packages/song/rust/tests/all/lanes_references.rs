// lanes_references.rs — `check_references` (decisions 259 to 271): every fault kind, cycles once by components, one fault list, no throw.
// A reader would get wrong that a fault is compared by `key()` and a message never: kind, holder and target are the whole answer.

use crate::lanes_support;
use crate::support;

use lanes_support::{inputs, lane};
use song::lanes::inputs::Inputs;
use song::lanes::references::{Checked, FaultKind, check_references};
use song::{Severity, Source};
use support::mem;

fn checked_with(i: &Inputs, source: &dyn Source) -> Checked {
    check_references(source, i, &i.lanes()).expect("checked")
}

/// The sorted fault keys of a project laid out in memory (its audio files are the `*.wav` names given).
fn keys_of(files: &[(&str, &str)]) -> Vec<String> {
    let i = inputs(files);
    let owned = lanes_support::laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    let mut keys: Vec<String> = checked_with(&i, &src).faults.iter().map(|f| f.key()).collect();
    keys.sort();
    keys
}

const CLIP: (&str, &str) = ("clips/kick.k7x", "source(\"clips/kick.wav\")\n");
const WAV: (&str, &str) = ("clips/kick.wav", "RIFF");
const RIFF: (&str, &str) = ("sequences/riff.k7x", "notes([t(0).d(1).k(60)])\n");

fn drums(events: &str) -> (&'static str, String) {
    ("nodes/drums.k7x", lane(&format!("events([{events}])\n")))
}

fn keys_with_drums(events: &str, extra: &[(&str, &str)]) -> Vec<String> {
    let d = drums(events);
    let mut files = vec![(d.0, d.1.as_str())];
    files.extend_from_slice(extra);
    keys_of(&files)
}

// A project that is right has no fault.
#[test]
fn a_clean_project_has_no_fault() {
    let keys = keys_with_drums(
        "ref(\"kick\").start(\"1:1\"), ref(\"riff\").start(\"2:1\")",
        &[CLIP, WAV, RIFF],
    );
    assert!(keys.is_empty(), "{keys:?}");
}

// inputs.ts: a ref that is neither clip nor sequence is not found, in the lane that names it (kind ref_missing).
#[test]
fn a_ref_that_is_neither_a_clip_nor_a_sequence_is_missing() {
    assert_eq!(
        keys_with_drums("ref(\"nope\").start(\"1:1\")", &[]),
        ["ref_missing|track:drums|nope"]
    );
}

// clipWindow.test.ts both "does not masquerade": a rejected clip or sequence is broken, never "not found".
#[test]
fn a_broken_clip_or_sequence_is_not_a_missing_ref_and_an_unplaced_one_is_no_fault() {
    let bad_clip = ("clips/kick.k7x", "source(\n");
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\")", &[bad_clip]),
        ["clip_broken|track:drums|kick"]
    );
    let bad_seq = ("sequences/riff.k7x", "notes(\n");
    assert_eq!(
        keys_with_drums("ref(\"riff\").start(\"1:1\")", &[bad_seq]),
        ["sequence_broken|track:drums|riff"]
    );
    // Nothing places the broken clip: it is no fault, and the family keeps its finding.
    assert!(keys_with_drums("", &[bad_clip, bad_seq]).is_empty());
    // 265: the file must be EXACTLY the ref's: `kick.v2` is not `kick`.
    let neighbour = ("clips/kick.v2.k7x", "source(\n");
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\")", &[neighbour]),
        ["ref_missing|track:drums|kick"]
    );
    // Two lanes naming one broken clip are a fault each.
    let two = keys_of(&[
        ("nodes/a.k7x", &lane("events([ref(\"kick\").start(\"1:1\")])\n")),
        ("nodes/b.k7x", &lane("events([ref(\"kick\").start(\"1:1\")])\n")),
        bad_clip,
    ]);
    assert_eq!(two, ["clip_broken|track:a|kick", "clip_broken|track:b|kick"]);
}

// A clip and a sequence of one name: the clip is checked, the sequence is never "missing".
#[test]
fn a_clip_and_a_sequence_of_one_name_are_checked_as_the_clip() {
    let seq = ("sequences/kick.k7x", "notes([t(0).d(1).k(60)])\n");
    assert!(keys_with_drums("ref(\"kick\").start(\"1:1\")", &[CLIP, WAV, seq]).is_empty());
    // The clip's file is missing: the sequence does not rescue it.
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\")", &[CLIP, seq, ("clips/other.wav", "x")]),
        ["audio_missing|track:drums|kick"]
    );
}

// audio: a file outside the mounts is outside, an absent one is missing (263).
#[test]
fn an_audio_file_outside_or_absent_is_named() {
    let outside = ("clips/kick.k7x", "source(\"/etc/passwd\")\n");
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\")", &[outside]),
        ["audio_outside|track:drums|kick"]
    );
    let other = ("clips/other.wav", "x");
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\")", &[CLIP, other]),
        ["audio_missing|track:drums|kick"]
    );
}

// A trim is a clip gain, which only audio has; repeating needs a known end.
#[test]
fn a_gain_on_a_sequence_and_a_loop_without_an_end_are_faults() {
    assert_eq!(
        keys_with_drums("ref(\"riff\").start(\"1:1\").gain(3)", &[RIFF]),
        ["sequence_gain|track:drums|riff"]
    );
    assert_eq!(
        keys_with_drums("ref(\"kick\").start(\"1:1\").loop(2)", &[CLIP, WAV]),
        ["loop_no_to|track:drums|kick"]
    );
    let ends = ("clips/kick.k7x", "source(\"clips/kick.wav\")\nto(2)\n");
    assert!(keys_with_drums("ref(\"kick\").start(\"1:1\").loop(2)", &[ends, WAV]).is_empty());
    assert!(keys_with_drums("ref(\"kick\").start(\"1:1\").loop(2).to(2)", &[CLIP, WAV]).is_empty());
    assert!(keys_with_drums("ref(\"kick\").start(\"1:1\").loop(1)", &[CLIP, WAV]).is_empty());
}

fn timeline(lanes: &str) -> String {
    format!("length(\"2:0\")\nlanes([{lanes}])\n")
}

fn tl_lane(name: &str, events: &str) -> String {
    format!("lane(\"{name}\").events([{events}])")
}

// timelines.test.ts "refuses a timeline placed inside itself" and 261: a self-loop is a cycle alone.
#[test]
fn a_timeline_in_itself_is_a_cycle_and_nothing_else() {
    let x = timeline(&tl_lane("a", "ref(\"x\").start(\"1:1\").loop(2)"));
    assert_eq!(keys_of(&[("timelines/x.k7x", &x)]), ["timeline_cycle|timeline:x:a|x"]);
}

// 261: a two-cycle, and a three-cycle with a chain into it from outside; the chain and a lane are not on the cycle.
#[test]
fn cycles_are_found_by_components_and_the_edge_into_one_is_no_cycle() {
    let x = timeline(&tl_lane("a", "ref(\"y\").start(\"1:1\")"));
    let y = timeline(&tl_lane("b", "ref(\"x\").start(\"1:1\")"));
    assert_eq!(
        keys_of(&[("timelines/x.k7x", &x), ("timelines/y.k7x", &y)]),
        ["timeline_cycle|timeline:x:a|y", "timeline_cycle|timeline:y:b|x"]
    );
    let (a, b, c) = (
        timeline(&tl_lane("l", "ref(\"b\").start(\"1:1\")")),
        timeline(&tl_lane("l", "ref(\"c\").start(\"1:1\")")),
        timeline(&tl_lane("l", "ref(\"a\").start(\"1:1\")")),
    );
    let outside = timeline(&tl_lane("l", "ref(\"a\").start(\"1:1\")"));
    let d = drums("ref(\"a\").start(\"1:1\")");
    let keys = keys_of(&[
        ("timelines/a.k7x", &a),
        ("timelines/b.k7x", &b),
        ("timelines/c.k7x", &c),
        ("timelines/w.k7x", &outside),
        (d.0, &d.1),
    ]);
    assert_eq!(
        keys,
        [
            "timeline_cycle|timeline:a:l|b",
            "timeline_cycle|timeline:b:l|c",
            "timeline_cycle|timeline:c:l|a",
        ]
    );
}

// timelines.test.ts "refuses a window, a loop or a fade": a fixed field is a fault; on an edge of a cycle both are reported.
#[test]
fn a_fixed_field_on_a_timeline_event_is_a_fault_and_both_on_a_cycle_edge() {
    let verse = timeline(&tl_lane("a", ""));
    let d = drums("ref(\"verse\").start(\"1:1\").loop(2)");
    assert_eq!(
        keys_of(&[("timelines/verse.k7x", &verse), (d.0, &d.1)]),
        ["timeline_fixed|track:drums|verse"]
    );
    let x = timeline(&tl_lane("a", "ref(\"y\").start(\"1:1\").fadeIn(1)"));
    let y = timeline(&tl_lane("b", "ref(\"x\").start(\"1:1\")"));
    assert_eq!(
        keys_of(&[("timelines/x.k7x", &x), ("timelines/y.k7x", &y)]),
        [
            "timeline_cycle|timeline:x:a|y",
            "timeline_cycle|timeline:y:b|x",
            "timeline_fixed|timeline:x:a|y",
        ]
    );
}

// timelines.test.ts "names a missing ref inside a timeline by its lane"; a timeline's pictures are not checked.
#[test]
fn a_missing_ref_inside_a_timeline_names_the_timeline_and_its_lane() {
    let verse = timeline(&tl_lane("x", "ref(\"nope\").start(\"1:1\")"));
    assert_eq!(
        keys_of(&[("timelines/verse.k7x", &verse)]),
        ["ref_missing|timeline:verse:x|nope"]
    );
    let pics = "length(\"2:0\")\nlanes([])\npictures([lane(\"v\").events([ref(\"nope\").start(\"1:1\")])])\n";
    assert!(keys_of(&[("timelines/verse.k7x", pics)]).is_empty());
}

// A timeline that does not read is a fault named by its file.
#[test]
fn a_broken_timeline_is_a_fault_named_by_its_file() {
    assert_eq!(
        keys_of(&[("timelines/bad.k7x", "lanes(\n")]),
        ["timeline_broken|file:timelines/bad.k7x|"]
    );
}

// 264: a strudel file that is not there is missing; one the Source refuses is no fault.
#[test]
fn a_pattern_file_that_is_absent_is_missing_and_a_climb_is_no_fault() {
    let kit = (
        "nodes/kit.k7x",
        "extends(\"@meta/strudel-track\")\nstrudel(\"algo/beat.strudel\")\n",
    );
    let other = ("algo/other.strudel", "s(\"bd\")\n");
    assert_eq!(keys_of(&[kit, other]), ["strudel_missing|track:kit|algo/beat.strudel"]);
    assert!(keys_of(&[kit, ("algo/beat.strudel", "s(\"bd\")\n")]).is_empty());
    let climb = (
        "nodes/kit.k7x",
        "extends(\"@meta/strudel-track\")\nstrudel(\"algo/../../../../../../x.strudel\")\n",
    );
    assert!(keys_of(&[climb, other]).is_empty());
}

const AMP: &str = "extends(\"@k7-core/audio/amp\")\n";
const AMP_MODULE: (&str, &str) = (
    "lib/k7-core/modules/audio/amp/module.k7x",
    "plugin({ type: \"internal\", name: \"Amp\" })\n",
);

// 55 and 265 / libraryModuleIssues.test.ts: an `extends` naming nothing is unknown, naming a file that did not read is broken.
#[test]
fn an_extends_is_unknown_or_broken_and_a_missing_module_is_silent() {
    let graph = ("graph.k7x", "modules({ mic: [\"amp\"], amp: [\"OUT\"] })\n");
    let mic = ("nodes/mic.k7x", "extends(\"@meta/track\")\n");
    let unknown = ("nodes/amp.k7x", "extends(\"@k7-core/audio/ghost\")\n");
    assert_eq!(
        keys_of(&[graph, mic, unknown]),
        ["extends_unknown|module:amp|@k7-core/audio/ghost"]
    );
    let broken = ("lib/k7-core/modules/audio/bad/module.k7x", "plugin(\n");
    let node = ("nodes/amp.k7x", "extends(\"@k7-core/audio/bad\")\n");
    assert_eq!(
        keys_of(&[graph, mic, node, broken]),
        ["extends_broken|module:amp|@k7-core/audio/bad"]
    );
    // A step naming no module at all compiles to nothing: no fault.
    assert!(keys_of(&[graph, mic]).is_empty());
    assert!(keys_of(&[graph, mic, ("nodes/amp.k7x", AMP), AMP_MODULE]).is_empty());
}

// 259: a finding is an ERROR at 1:1 on the holder's file, short, and quotes a long word cut.
#[test]
fn a_fault_is_one_error_finding_on_its_holders_file_with_a_short_message() {
    let long = "x".repeat(1_000_000);
    let d = drums(&format!("ref(\"{long}\").start(\"1:1\")"));
    let i = inputs(&[(d.0, &d.1)]);
    let c = checked_with(&i, &mem(&[]));
    assert_eq!(c.faults.len(), 1);
    assert!(c.faults[0].target.chars().count() <= 200);
    let f = &c.findings[0];
    assert_eq!(
        (f.file.as_str(), f.line, f.col, f.severity),
        ("nodes/drums.k7x", 1, 1, Severity::Error)
    );
    assert!(f.message.len() <= 300, "{}", f.message.len());
    assert_eq!(c.faults[0].kind, FaultKind::RefMissing);
    // A timeline's file, a vocabulary module's and a chain's.
    let t = timeline(&tl_lane("l", "ref(\"nope\").start(\"1:1\")"));
    let i = inputs(&[("timelines/verse.k7x", &t)]);
    let c = checked_with(&i, &mem(&[]));
    assert_eq!(c.findings[0].file, "timelines/verse.k7x");
}

// 271: a finding names the file that holds the fault: a node's, a vocabulary module's, a timeline's, `project.k7x` or the routing's.
#[test]
fn each_holder_is_named_by_its_own_file() {
    use song::lanes::references::{Fault, Holder, finding_of};
    let t = timeline(&tl_lane("l", ""));
    let i = inputs(&[
        ("nodes/amp.k7x", "plugin({ type: \"internal\", name: \"Chorus\" })\n"),
        ("nodes/a.k7x", &lane("")),
        ("graph.k7x", "modules({ a: [\"amp\"], amp: [\"OUT\"] })\n"),
        ("timelines/verse.k7x", &t),
    ]);
    let file = |k: FaultKind, h: Holder| finding_of(&i, &Fault::new(k, h, "x")).file;
    assert_eq!(
        file(FaultKind::ExtendsUnknown, Holder::Module("amp".into())),
        "nodes/amp.k7x"
    );
    assert_eq!(
        file(FaultKind::ExtendsUnknown, Holder::Module("@k7-core/audio/amp".into())),
        "@k7-core/audio/amp/module.k7x"
    );
    assert_eq!(
        file(FaultKind::TapUnrouted, Holder::Node("amp".into())),
        "nodes/amp.k7x"
    );
    // A tap fault on a vocabulary module names its module file, as a `Module` holder does (271).
    assert_eq!(
        file(FaultKind::TapUnrouted, Holder::Node("@k7-core/audio/kit".into())),
        "@k7-core/audio/kit/module.k7x"
    );
    assert_eq!(file(FaultKind::RefMissing, Holder::Track("a".into())), "nodes/a.k7x");
    assert_eq!(
        file(
            FaultKind::RefMissing,
            Holder::Timeline {
                id: "verse".into(),
                lane: "l".into()
            }
        ),
        "timelines/verse.k7x"
    );
    assert_eq!(
        file(
            FaultKind::TooFew,
            Holder::Chain {
                name: "a".into(),
                step: Some(1)
            }
        ),
        "nodes/a.k7x"
    );
    // A chain whose key is no node names its first step's node, else the routing.
    assert_eq!(
        file(
            FaultKind::TooFew,
            Holder::Chain {
                name: "amp".into(),
                step: None
            }
        ),
        "nodes/amp.k7x"
    );
    assert_eq!(
        file(
            FaultKind::TooFew,
            Holder::Chain {
                name: "zz".into(),
                step: None
            }
        ),
        "graph.k7x"
    );
    assert_eq!(file(FaultKind::ConsoleAbsent, Holder::Console), "project.k7x");
    assert_eq!(
        file(FaultKind::TimelineBroken, Holder::File("timelines/bad.k7x".into())),
        "timelines/bad.k7x"
    );
    assert_eq!(file(FaultKind::More, Holder::File(String::new())), "project.k7x");
}

// 271: a timeline whose id is past the 200 characters a fault holds is still named by its own file.
#[test]
fn a_timeline_with_a_long_id_is_named_by_its_real_file() {
    let id = "t".repeat(230);
    let t = timeline(&tl_lane("l", "ref(\"nope\").start(\"1:1\")"));
    let path = format!("timelines/{id}.k7x");
    let i = inputs(&[(path.as_str(), t.as_str())]);
    let c = checked_with(&i, &mem(&[]));
    assert_eq!(c.faults.len(), 1);
    assert_eq!(c.findings[0].file, path);
}

// 285: a timeline that does not read is no clip or sequence, so an event placing it is a missing ref, as the TypeScript reads it.
#[test]
fn a_placed_timeline_that_does_not_read_is_a_missing_ref_beside_its_own_fault() {
    assert_eq!(
        keys_with_drums("ref(\"bad\").start(\"1:1\")", &[("timelines/bad.k7x", "lanes(\n")]),
        ["ref_missing|track:drums|bad", "timeline_broken|file:timelines/bad.k7x|"]
    );
}
