// project.rs — tempo, meter, markers and the dispatcher's own refusals on a real fixture: the root they write and the doc it compiles to.
// A reader would get wrong a marker's key: its canonical timecode, so `2:1` and `2:1:1` are the same marker.

mod support;

use support::Project;

#[test]
fn the_tempo_reaches_the_engine_to_two_decimals() {
    let mut p = Project::open("markers-settings");
    assert_eq!(p.doc().tempo, 96.0);
    let ran = p.round_trip("project tempo 128.456");
    assert_eq!(ran.answer.log, ["tempo 128.46 bpm"]);
    assert_eq!(ran.changed(&p.dir), ["project.k7x"]);
    p.ok("project tempo 128.456");
    assert_eq!(p.doc().tempo, 128.46);
    p.refused("project tempo 900", "a number from 20 to 400");
    p.refused("project tempo fast", "<bpm> expects a number");
}

#[test]
fn the_meter_changes_what_a_bar_is() {
    let mut p = Project::open("markers-settings");
    p.round_trip("project meter 6/8");
    p.ok("project meter 6/8");
    let doc = p.doc();
    assert_eq!((doc.time_sig_numerator, doc.time_sig_denominator), (6, 8));
    p.refused("project meter 7/3", "usage: project meter");
    p.refused("project meter 33/4", "usage: project meter");
}

#[test]
fn markers_are_added_renamed_and_deleted_by_name() {
    let mut p = Project::open("markers-settings");
    p.round_trip("project marker new 2:1 Verse");
    p.ok("project marker new 2:1 Verse");
    p.ok("project marker new 2:1:1 Bridge");
    let names: Vec<(String, String)> = p.doc().markers.iter().map(|m| (m.tc.clone(), m.name.clone())).collect();
    assert_eq!(
        names,
        [
            ("1:1:1".into(), "intro".into()),
            ("2:1".into(), "Bridge".into()),
            ("3:1:1".into(), "chorus".into())
        ]
    );
    let ran = p.round_trip("project marker delete chorus");
    assert_eq!(ran.answer.log, ["deleted marker \"chorus\""]);
    p.ok("project marker delete chorus");
    assert_eq!(p.doc().markers.len(), 2);
    p.refused("project marker delete nothing", "no marker named \"nothing\"");
    p.refused("project marker new x Bad", "expects a bar:beat:sixteenth position");
}

#[test]
fn the_dispatcher_refuses_before_any_verb() {
    let mut p = Project::open("clip-lane");
    p.refused("frobnicate", "unknown command: frobnicate");
    p.refused("event", "unknown command: event");
    let ported: Vec<&str> = verbs::ported().collect();
    let missing: Vec<&str> = verbs::catalogue::specs()
        .iter()
        .map(|s| s.sid.as_str())
        .filter(|s| !ported.contains(s))
        .collect();
    assert!(
        missing.is_empty(),
        "the catalogue names verbs with no handler: {missing:?}"
    );
    assert_eq!(ported.len(), 191);
    let ran = p.ok("project reload");
    assert_eq!(ran.answer.effects, ["reload"]);
}

#[test]
fn a_project_of_a_newer_format_is_edited_by_no_verb() {
    let mut p = Project::open("clip-lane");
    let path = format!("{}/project.k7x", p.dir);
    let text = format!("format(999)\n{}", p.text("project.k7x"));
    song_edit::Fs::write_atomic(&mut p.fs, &path, text.as_bytes()).expect("a write");
    p.refused("project tempo 100", "written by a newer k7");
}
