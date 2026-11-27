// files_validate_cases.rs — where the editor's check places a finding, and the event and project.layers rules the lane judge applies.
// A reader would get wrong that the reader answers its FIRST finding only: a case names the one it provokes.

use song::files::{ValidityDepth, validate_file};

const MODULE: &str = "// a node\nextends(\"@workspace/instruments/synth\")\nparams({\n  \"Filter Freq\": 40,\n  Level: -12,\n})\n";

#[test]
fn a_buffer_the_compiler_could_read_passes_at_schema_depth() {
    let v = validate_file("song/nodes/pad.k7x", MODULE);
    assert!(v.ok);
    assert_eq!(v.depth, ValidityDepth::Schema);
}

#[test]
fn a_syntax_error_is_reported_at_its_own_line_and_column() {
    let v = validate_file("song/nodes/pad.k7x", &format!("{MODULE}bogus(1)\noops"));
    assert!(!v.ok && v.malformed);
    assert_eq!(v.depth, ValidityDepth::Syntax);
    assert_eq!((v.issues[0].line, v.issues[0].col), (Some(8), Some(5)));
    assert_eq!(v.issues[0].message, "expected \"(\" after oops");
}

#[test]
fn a_schema_finding_is_on_the_line_of_the_key_it_is_about() {
    let v = validate_file("song/nodes/pad.k7x", &format!("{MODULE}nonsense(1)\n"));
    assert!(!v.ok);
    assert_eq!(v.issues.len(), 1);
    let i = &v.issues[0];
    assert_eq!((i.line, i.col, i.path.as_str()), (Some(7), Some(1), "nonsense"));
}

#[test]
fn a_nested_finding_is_placed_inside_its_object() {
    let v = validate_file("song/project.k7x", "project({\n  name: \"demo\",\n  tempo: \"fast\",\n})\n");
    assert!(!v.ok);
    let i = &v.issues[0];
    assert_eq!((i.path.as_str(), i.line, i.col), ("project.tempo", Some(3), Some(3)));
}

#[test]
fn a_missing_key_is_named_by_its_path_at_the_object_that_lacks_it() {
    let v = validate_file("song/sequences/riff.k7x", "to(\"2:1\")\n");
    assert_eq!(v.issues[0].path, "notes");
    assert!(v.issues[0].message.contains("missing key"), "{}", v.issues[0].message);
}

#[test]
fn a_lanes_node_file_is_checked_as_a_module_file_at_schema_depth() {
    let v = validate_file("tracks/gtr.k7x", "extends(3)\n");
    assert_eq!(v.depth, ValidityDepth::Schema);
    assert!(!v.ok);
}

fn root(layers: &str) -> song::files::Validity {
    validate_file("project.k7x", &format!("project({{ name: \"s\", tempo: 120, layers: {layers} }})\n"))
}

#[test]
fn project_layers_is_accepted_empty_and_refused_naming_what_draws_the_picture_now() {
    assert!(root("[]").ok);
    let r = root("[\"main\"]");
    assert!(!r.ok);
    assert!(r.issues[0].message.contains("trackOrder"), "{}", r.issues[0].message);
}

fn lane(event: &str) -> song::files::Validity {
    validate_file("nodes/lane.k7x", &format!("extends(\"@meta/picture-track\")\nevents([{event}])\n"))
}

fn message(event: &str) -> String {
    lane(event).issues.first().map(|i| i.message.clone()).unwrap_or_default()
}

#[test]
fn an_event_takes_an_audio_events_words_and_a_shots() {
    assert!(lane("{ ref: \"riff\", start: \"3:1\", fadeIn: \"0:1\", loop: 2 }").ok);
    assert!(
        lane("{ ref: \"shot\", start: 4, length: 3, from: 1.5, speed: 2, fadeIn: 0.5, fadeOut: { over: 1, with: \"wipe\" }, animate: { x: [{ t: 0, v: 0 }] }, silent: true }")
            .ok
    );
}

#[test]
fn no_ref_is_only_an_activation_with_nothing_that_needs_a_source() {
    assert!(lane("{ start: 2, length: 4 }").ok);
    assert!(message("{ start: 2, from: 1 }").contains("ACTIVATION"));
}

#[test]
fn a_backwards_window_and_a_reversed_shot_that_runs_out_of_source_are_refused() {
    assert!(message("{ ref: \"a\", from: 2, to: 1 }").contains("greater than 'from'"));
    assert!(message("{ ref: \"a\", from: 1, speed: -1, length: 3 }").contains("runs out of source"));
}

#[test]
fn a_key_neither_side_has_is_still_refused() {
    assert!(!lane("{ ref: \"a\", source_from: 1 }").ok);
}

#[test]
fn an_event_starts_at_a_timecode_or_a_non_negative_number() {
    assert!(lane("{ ref: \"a\", start: 4.5 }").ok);
    assert!(lane("{ ref: \"a\", start: \"3:1\" }").ok);
    assert!(!lane("{ ref: \"a\", start: -1 }").ok);
    assert!(!lane("{ ref: \"a\", start: \"4.5s\" }").ok);
}

#[test]
fn a_shot_takes_any_finite_speed_backwards_included_and_an_activation_none() {
    assert!(lane("{ ref: \"a\", start: 0, length: 2, speed: 0.5 }").ok);
    assert!(lane("{ ref: \"a\", start: 0, length: 2, speed: 0 }").ok);
    assert!(lane("{ ref: \"a\", start: 0, length: 2, from: 3, speed: -1 }").ok);
    assert!(!lane("{ ref: \"a\", start: 0, length: 4, from: 3, speed: -1 }").ok);
    assert!(!lane("{ start: 0, length: 2, speed: 2 }").ok);
}
