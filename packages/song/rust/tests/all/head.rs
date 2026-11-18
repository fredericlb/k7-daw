// head.rs — project.k7x and settings.k7x: the format level's refusal, the markers in beat order, the settings defaulted.
// A reader would get wrong that a newer format refuses before any parse: its later syntax may be one this build cannot read.

use crate::support;

use song::{Cache, Head, read_head};
use support::mem;

fn head(files: &[(&str, &str)]) -> Head {
    read_head(&mem(files), &mut Cache::new(), "/p").expect("reads")
}

#[test]
fn refuses_a_newer_root_before_any_schema() {
    // projectFormat.test.ts: refuses a newer root (validate true and false).
    let h = head(&[("project.k7x", "format(6)\nproject({ tempo: 120 })\n")]);
    assert!(h.format_too_new);
    assert_eq!((h.findings[0].file.as_str(), h.findings[0].line), ("project.k7x", 1));
    assert!(h.markers.is_empty());
}

#[test]
fn refuses_a_newer_root_whose_later_syntax_this_build_cannot_parse() {
    // projectFormat.test.ts: refuses a newer root whose later syntax this build cannot parse.
    let text = "// the level is on line two\nformat(6)\nproject(tempo => 120)\n";
    assert!(k7x::parse_tree(text, &k7x::Bump::new()).is_err());
    let h = head(&[("project.k7x", text)]);
    assert!(h.format_too_new);
    assert_eq!(h.findings[0].line, 2);
}

#[test]
fn names_the_level_in_the_message_and_the_file() {
    // projectFormat.test.ts: names the file and both levels.
    let h = head(&[("project.k7x", "format(7)\n")]);
    let root = h.root.expect("a root");
    let too_new = root.too_new.as_ref().expect("refused");
    assert_eq!(too_new.level, 7);
    assert!(h.findings[0].message.contains("format 7") && h.findings[0].message.contains("format 5"));
}

#[test]
fn a_level_written_as_data_is_refused_too() {
    let h = head(&[("project.k7x", "project({ tempo: 1 })\nformat: 9\n")]);
    assert!(h.format_too_new || !h.findings.is_empty());
}

#[test]
fn loads_the_current_level_written_or_absent() {
    // projectFormat.test.ts: loads the current level, written or absent.
    let h = head(&[("project.k7x", "format(5)\nproject({ tempo: 97 })\n")]);
    assert!(!h.format_too_new && h.findings.is_empty());
    let project = h.root.as_ref().and_then(|r| r.project.as_ref()).expect("a project");
    assert_eq!(project.project.as_ref().and_then(|p| p.tempo).map(|t| t.0), Some(97.0));
    let h = head(&[("project.k7x", "project({ tempo: 98 })\n")]);
    assert!(h.findings.is_empty());
}

#[test]
fn no_root_is_no_head() {
    let h = head(&[("a.k7x", "x(1)\n")]);
    assert!(h.root.is_none() && h.findings.is_empty() && h.markers.is_empty());
}

#[test]
fn markers_come_out_in_beat_order_with_their_beat() {
    let text = "markers({ \"5:1:1\": \"chorus\", \"1:1:1\": \"intro\", \"3:1\": \"verse\" })\n";
    let h = head(&[("project.k7x", text)]);
    let tcs: Vec<(&str, f64)> = h.markers.iter().map(|m| (m.tc.as_str(), m.beat)).collect();
    assert_eq!(tcs, [("1:1:1", 0.0), ("3:1", 8.0), ("5:1:1", 16.0)]);
}

#[test]
fn a_marker_reads_its_position_against_the_time_signature() {
    let text = "project({ timeSignature: \"3/4\" })\nmarkers({ \"2:1\": \"b\" })\n";
    let h = head(&[("project.k7x", text)]);
    assert_eq!(h.markers[0].beat, 3.0);
}

#[test]
fn a_bad_marker_key_and_the_dead_type_keys_are_findings() {
    // The dead type keys are unknown fields; a v1 wall-clock key has the timecode shape, so only a non-timecode key is refused.
    for text in [
        "markers({ intro: \"x\" })\n",
        "type(\"audio\")\n",
        "project_type(\"audio\")\n",
        "project({ foo: 1 })\n",
        "project({ tempo: 0 })\n",
        "project({ timeSignature: \"fast\" })\n",
        "project({ layers: [\"a\"] })\n",
        "project({ console: \"nope\" })\n",
        "trackOrder(3)\n",
    ] {
        let h = head(&[("project.k7x", text)]);
        assert_eq!(h.findings.len(), 1, "{text}");
        assert_eq!(h.findings[0].file, "project.k7x");
    }
}

#[test]
fn the_root_reads_the_two_tiers_and_the_lists() {
    let text = "format(5)\nproject({ name: \"s\", tempo: 90, sampleRate: 44100, timeSignature: \"6/8\", masterVolume: -3,\n  viewport: [1920, 1080], layers: [], tags: [\"a\"], video: true, modules_from: [\"x\"] })\n\
        trackOrder([\"b\", \"a\"])\nsoloed([\"a\"])\nhighlighted([])\n";
    let h = head(&[("project.k7x", text)]);
    assert!(h.findings.is_empty(), "{:?}", h.findings);
    let file = h.root.as_ref().and_then(|r| r.project.as_ref()).expect("a project");
    assert_eq!(file.track_order.as_deref(), Some(&["b".to_owned(), "a".to_owned()][..]));
    let json = serde_json::to_value(file).expect("json");
    assert_eq!(json["project"]["timeSignature"], "6/8");
    assert_eq!(json["trackOrder"][0], "b");
}

#[test]
fn settings_default_each_field() {
    // metronomeLevel.test.ts: defaults where the file says nothing; the level and the flag are kept apart.
    let h = head(&[]);
    assert_eq!(
        (h.settings.metronome, h.settings.metronome_level, h.settings.count_in),
        (false, 0.6, 1)
    );
    let h = head(&[("settings.k7x", "metronome(false)\nmetronome_level(0.9)\n")]);
    assert_eq!((h.settings.metronome, h.settings.metronome_level), (false, 0.9));
}

#[test]
fn settings_read_as_written_and_a_stated_zero_count_in_is_zero() {
    // metronomeLevel.test.ts: is read as written.
    let h = head(&[(
        "settings.k7x",
        "metronome(true)\nmetronome_level(0.4)\ncount_in(0)\nanything(\"kept\")\n",
    )]);
    assert_eq!(
        (h.settings.metronome, h.settings.metronome_level, h.settings.count_in),
        (true, 0.4, 0)
    );
    assert!(h.findings.is_empty());
}

#[test]
fn the_schema_refuses_a_level_out_of_range_rather_than_clamping_it() {
    // metronomeLevel.test.ts: is refused outright by the schema when out of range.
    assert!(head(&[("settings.k7x", "metronome_level(0.5)\n")]).findings.is_empty());
    for text in [
        "metronome_level(0)\n",
        "metronome_level(1.5)\n",
        "metronome_level(\"loud\")\n",
        "count_in(-1)\n",
        "count_in(1.5)\n",
        "metronome(1)\n",
    ] {
        let h = head(&[("settings.k7x", text)]);
        assert_eq!(h.findings.len(), 1, "{text}");
        assert_eq!(h.findings[0].file, "settings.k7x");
        assert_eq!(h.settings.metronome_level, 0.6);
    }
}
