// The cards and cables a build draws: the wires, the curves bound to them, the signal on each cable, a pass-through, a stale `from`.

use super::*;
use serde_json::json;

fn one(g: Value) -> Built {
    built(&g, json!({}))
}

#[test]
fn wires_track_module_sink_for_a_one_module_chain() {
    let b = one(doc(
        json!({"lead": {"from": "melody", "use": [{"module": "synth"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["melody"]),
    ));
    assert_eq!(b.node("trk:melody")["kind"], "track");
    let synth = b.node("mod:synth");
    assert_eq!(
        (&synth["kind"], &synth["label"], &synth["chain"], &synth["chains"]),
        (&json!("module"), &json!("synth"), &json!("lead"), &json!(["lead"]))
    );
    assert_eq!(b.node("snk:OUT")["kind"], "sink");
    assert_eq!(b.pairs(), ["mod:synth->snk:OUT", "trk:melody->mod:synth"]);
}

#[test]
fn labels_an_out_edge_with_its_non_unity_gain_as_a_percentage() {
    let b = one(doc(
        json!({"a": {"use": [{"module": "x"}], "out": [{"to": "OUT", "gain": 0.3}]}}),
        json!([]),
    ));
    let to_out = b.edges.iter().find(|e| e["target"] == "snk:OUT").expect("a cable");
    assert_eq!(to_out["label"], "30%");
}

#[test]
fn renders_an_empty_chain_as_a_single_pass_through_node() {
    let b = one(doc(
        json!({"bus": {"use": [], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
    ));
    assert_eq!(b.node("chn:bus")["kind"], "chain");
}

#[test]
fn adds_a_curve_node_and_an_automation_edge_per_binding_targeting_a_widget() {
    let ui =
        json!({"cols": 2, "rows": 1, "widgets": {"cut": {"type": "knob", "col": 0, "row": 0, "bind": "Filter Freq"}}});
    let g = doc_with(
        json!({"lead": {"use": [{"module": "synth", "ui": ui, "automations": [
            {"param": "Filter Freq", "curve": "sweep"},
            {"param": "Osc1 Tune", "curve": "wobble"}
        ]}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
        json!({"curves": {"sweep": {"type": "custom"}, "wobble": {"type": "lfo", "shape": "triangle"}}}),
    );
    let b = one(g);
    let sweep = b.node("crv:sweep@synth:Filter Freq");
    assert_eq!(
        (&sweep["kind"], &sweep["label"], &sweep["sub"]),
        (&json!("curve"), &json!("sweep"), &json!("custom"))
    );
    let wobble = b.node("crv:wobble@synth:Osc1 Tune");
    assert_eq!(
        (&wobble["kind"], &wobble["label"], &wobble["sub"]),
        (&json!("curve"), &json!("wobble"), &json!("lfo triangle"))
    );
    let with_widget = b
        .edges
        .iter()
        .find(|e| text(&e["source"]).starts_with("crv:sweep@"))
        .expect("an edge");
    let no_widget = b
        .edges
        .iter()
        .find(|e| text(&e["source"]).starts_with("crv:wobble@"))
        .expect("an edge");
    assert_eq!(with_widget["target"], "mod:synth");
    assert_eq!(with_widget["automation"]["param"], "Filter Freq");
    assert!(with_widget["automation"]["widget"].is_object());
    assert_eq!(no_widget["automation"]["param"], "Osc1 Tune");
    assert!(no_widget["automation"]["widget"].is_null());
}

#[test]
fn a_curve_driving_two_params_is_two_nodes_one_per_binding() {
    let g = doc_with(
        json!({"lead": {"use": [{"module": "synth", "automations": [
            {"param": "Filter Freq", "curve": "sweep"}, {"param": "Osc1 Tune", "curve": "sweep"}
        ]}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
        json!({"curves": {"sweep": {"type": "custom"}}}),
    );
    let b = one(g);
    let curves = b.of_kind("curve");
    assert_eq!(curves.len(), 2);
    assert_eq!(
        b.edges
            .iter()
            .filter(|e| text(&e["source"]).starts_with("crv:sweep@"))
            .count(),
        2
    );
    assert_eq!(
        curves.iter().map(|n| text(&n["label"])).collect::<Vec<_>>(),
        ["sweep", "sweep"]
    );
}

#[test]
fn a_curve_an_event_condition_starts_gets_the_nodes_note_on_and_one_that_runs_always_none() {
    let lfo =
        json!({"type": "lfo", "polarity": "unipolar", "amplitude": 1, "shape": "saw", "frequency": 1, "phase": 0});
    let mut on_note = lfo.clone();
    on_note["conditions"] = json!([{"type": "event", "event": "note-on", "retrigger": true}]);
    let g = doc_with(
        json!({"lead": {"use": [{"module": "synth", "automations": [
            {"param": "Cut", "curve": "onNote"}, {"param": "Res", "curve": "free"}
        ]}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
        json!({"curves": {"onNote": {"type": "lfo", "spec": on_note}, "free": {"type": "lfo", "spec": lfo}}}),
    );
    let b = one(g);
    let fired: Vec<&Value> = b.edges.iter().filter(|e| e["signal"] == "event").collect();
    assert_eq!(fired.len(), 1);
    assert_eq!(
        (&fired[0]["source"], &fired[0]["target"], &fired[0]["sourcePort"]),
        (&json!("mod:synth"), &json!("crv:onNote@synth:Cut"), &json!("note-on"))
    );
    assert_eq!(b.node("crv:onNote@synth:Cut")["eventIn"], true);
    assert_eq!(b.node("crv:free@synth:Res")["eventIn"], false);
}

#[test]
fn skips_an_automation_whose_curve_is_missing_from_the_graph() {
    let g = doc_with(
        json!({"lead": {"use": [{"module": "s", "automations": [{"param": "P", "curve": "gone"}]}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
        json!({"curves": {}}),
    );
    assert!(one(g).of_kind("curve").is_empty());
}

#[test]
fn keeps_the_track_input_when_a_chain_is_named_after_its_source_track() {
    let b = one(doc(
        json!({"drums": {"from": "drums", "use": [], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["drums"]),
    ));
    assert_eq!(b.node("trk:drums")["kind"], "track");
    assert_eq!(b.pairs(), ["chn:drums->snk:OUT", "trk:drums->chn:drums"]);
}

#[test]
fn keeps_two_cables_when_one_source_feeds_a_target_from_two_ports() {
    let b = one(doc(
        json!({
            "sep": {"from": "stems", "use": [], "out": [
                {"to": "mix", "gain": 1, "fromPort": "drums"}, {"to": "mix", "gain": 1, "fromPort": "bass"}]},
            "mix": {"use": [], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!([]),
    ));
    let to_mix: Vec<&Value> = b.edges.iter().filter(|e| e["target"] == "chn:mix").collect();
    assert_eq!(to_mix.len(), 2);
    let mut ports: Vec<String> = to_mix.iter().map(|e| text(&e["sourcePort"])).collect();
    ports.sort();
    assert_eq!(ports, ["bass", "drums"]);
    let ids: HashSet<String> = to_mix.iter().map(|e| text(&e["id"])).collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn collapses_a_link_expressed_from_both_ends_and_keeps_its_port() {
    let b = one(doc(
        json!({
            "a": {"use": [], "out": [{"to": "b", "gain": 1, "fromPort": "left"}]},
            "b": {"from": "a", "use": [], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!([]),
    ));
    let ab: Vec<&Value> = b
        .edges
        .iter()
        .filter(|e| e["source"] == "chn:a" && e["target"] == "chn:b")
        .collect();
    assert_eq!(ab.len(), 1);
    assert_eq!(ab[0]["sourcePort"], "left");
}

fn signals(b: &Built) -> Vec<(String, Option<String>)> {
    let mut s: Vec<(String, Option<String>)> = b
        .edges
        .iter()
        .map(|e| (pair(e), e["signal"].as_str().map(str::to_owned)))
        .collect();
    s.sort();
    s
}

fn sig(b: &Built, key: &str) -> Option<String> {
    signals(b).into_iter().find(|(k, _)| k == key).and_then(|(_, s)| s)
}

#[test]
fn turns_midi_into_audio_across_an_instrument() {
    let g = doc_with(
        json!({"lead": {"from": "melody", "use": [
            {"module": "synth", "emits": "audio"}, {"module": "reverb", "emits": "audio"}
        ], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["melody"]),
        json!({"trackSignals": {"melody": "midi"}}),
    );
    let want = [
        ("mod:reverb->snk:OUT", "audio"),
        ("mod:synth->mod:reverb", "audio"),
        ("trk:melody->mod:synth", "midi"),
    ];
    let got = signals(&one(g));
    assert_eq!(got.len(), 3);
    for (k, s) in want {
        assert!(got.contains(&(k.to_owned(), Some(s.to_owned()))), "{k}");
    }
}

#[test]
fn draws_a_split_lanes_two_cables_from_their_own_jacks() {
    let g = doc_with(
        json!({
            "gtr#audio": {"from": "gtr", "fromPort": "audio", "use": [{"module": "amp", "emits": "audio"}], "out": [{"to": "OUT", "gain": 1}]},
            "gtr#midi": {"from": "gtr", "fromPort": "midi", "use": [{"module": "synth", "emits": "audio"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["gtr"]),
        json!({"trackSignals": {"gtr": "audio"}}),
    );
    let b = one(g);
    assert_eq!(sig(&b, "trk:gtr->mod:amp").as_deref(), Some("audio"));
    assert_eq!(sig(&b, "trk:gtr->mod:synth").as_deref(), Some("midi"));
    let jack = |target: &str| {
        b.edges
            .iter()
            .find(|e| e["source"] == "trk:gtr" && e["target"] == target)
            .map(|e| e["sourcePort"].clone())
    };
    assert_eq!(jack("mod:amp"), Some(json!("audio")));
    assert_eq!(jack("mod:synth"), Some(json!("midi")));
}

#[test]
fn keeps_a_midi_transformers_output_midi() {
    let g = doc_with(
        json!({"lead": {"from": "keys", "use": [
            {"module": "arp", "emits": "midi"}, {"module": "synth", "emits": "audio"}
        ], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["keys"]),
        json!({"trackSignals": {"keys": "midi"}}),
    );
    let b = one(g);
    assert_eq!(sig(&b, "trk:keys->mod:arp").as_deref(), Some("midi"));
    assert_eq!(sig(&b, "mod:arp->mod:synth").as_deref(), Some("midi"));
    assert_eq!(sig(&b, "mod:synth->snk:OUT").as_deref(), Some("audio"));
}

#[test]
fn carries_video_through_a_video_chain() {
    let g = doc_with(
        json!({"pic": {"from": "cam", "use": [{"module": "blur", "emits": "video"}], "out": [{"to": "VIDEO_OUT", "gain": 1}]}}),
        json!(["cam"]),
        json!({"trackSignals": {"cam": "video"}}),
    );
    assert_eq!(sig(&one(g), "mod:blur->snk:VIDEO_OUT").as_deref(), Some("video"));
}

#[test]
fn types_a_chain_to_chain_send_from_the_upstream_chains_last_module() {
    let b = one(doc(
        json!({
            "lead": {"from": "vox", "use": [{"module": "comp", "emits": "audio"}], "out": [{"to": "reverb", "gain": 0.3}]},
            "reverb": {"use": [{"module": "conv", "emits": "audio"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!([]),
    ));
    assert_eq!(sig(&b, "mod:comp->mod:conv").as_deref(), Some("audio"));
}

#[test]
fn leaves_an_untyped_sources_cable_untyped_rather_than_guessing() {
    let b = one(doc(
        json!({"a": {"use": [{"module": "x"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
    ));
    assert_eq!(sig(&b, "mod:x->snk:OUT"), None);
}

#[test]
fn carries_the_sources_signal_across_a_chain_with_no_modules() {
    let g = doc_with(
        json!({"sc": {"from": "keys", "use": [], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["keys"]),
        json!({"trackSignals": {"keys": "midi"}}),
    );
    let b = one(g);
    let out = b.edges.iter().find(|e| e["target"] == "snk:OUT").expect("a cable");
    assert_eq!(out["signal"], "midi");
}

#[test]
fn does_not_hang_on_a_chain_cycle_while_following_a_pass_through() {
    let b = one(doc(
        json!({
            "a": {"from": "b", "use": [], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "a", "use": [], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!([]),
    ));
    assert!(b.edges.iter().all(|e| e["signal"].is_null()));
}

#[test]
fn a_chain_whose_source_track_no_longer_exists_still_draws_the_track_node() {
    let b = one(doc(
        json!({"vox": {"from": "vox", "use": [{"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
    ));
    assert!(b.ids().contains(&"trk:vox".to_owned()));
    assert!(b.pairs().contains(&"trk:vox->mod:eq".to_owned()));
}

#[test]
fn a_real_chain_to_chain_feed_is_still_read_as_one() {
    let b = one(doc(
        json!({
            "send": {"from": "lead", "use": [{"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]},
            "lead": {"from": "vox", "use": [{"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox"]),
    ));
    assert!(!b.ids().contains(&"trk:lead".to_owned()));
    assert!(b.pairs().contains(&"mod:eq->mod:verb".to_owned()));
}

#[test]
fn a_track_sharing_its_chains_name_resolves_to_the_track() {
    let b = one(doc(
        json!({"drums": {"from": "drums", "use": [{"module": "comp"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["drums"]),
    ));
    assert!(b.ids().contains(&"trk:drums".to_owned()));
}
