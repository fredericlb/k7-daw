// What a History row lights, pinned by which file lights what.

use super::*;
use serde_json::json;

fn graph() -> MarkGraph {
    serde_json::from_value(json!({
        "chains": {"be.bass": {"from": "be.bass", "use": [{"module": "5ee626e6"}, {"module": "fb4235d5"}], "out": []}},
        "tracks": ["be.bass"],
        "warnings": []
    }))
    .unwrap_or_default()
}

fn paths(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_lane_is_lit_for_its_track_file_and_a_clip_for_its_sequence_or_audio_file() {
    let m = history_marks(
        &paths(&["tracks/da.main.k7x", "sequences/irrupt_kit_5_1.k7x", "clips/take 3.wav"]),
        "",
        None,
    );
    assert_eq!(m.lanes, ["da.main"]);
    assert_eq!(m.clips, ["irrupt_kit_5_1", "take 3"]);
}

#[test]
fn only_the_node_a_journal_label_names_is_lit_inside_the_chain_it_changed() {
    let label = "node param \"5ee626e6\" \"model\" \"@k7-core/x.nam\"";
    let m = history_marks(&paths(&["chains/be.bass.k7x"]), label, Some(&graph()));
    assert_eq!(m.modules, ["5ee626e6"]);
    assert!(m.chains.is_empty());
}

#[test]
fn the_whole_chain_is_lit_when_nothing_names_a_node_a_commit_say() {
    let m = history_marks(&paths(&["chains/be.bass.k7x"]), "sync", Some(&graph()));
    assert_eq!(m.chains, ["be.bass"]);
    assert!(m.modules.is_empty());
}

#[test]
fn a_chain_the_graph_does_not_know_is_lit_whole() {
    let m = history_marks(&paths(&["chains/gone.k7x"]), "node param \"5ee626e6\"", Some(&graph()));
    assert_eq!(m.chains, ["gone"]);
}

#[test]
fn nothing_is_lit_for_project_level_files() {
    let m = history_marks(&paths(&["project.k7x", "layout.k7x"]), "", None);
    assert_eq!(m, HistoryMark::default());
}

#[test]
fn a_name_met_twice_is_listed_once() {
    let m = history_marks(&paths(&["tracks/a.k7x", "tracks/a.k7x"]), "", None);
    assert_eq!(m.lanes, ["a"]);
}
