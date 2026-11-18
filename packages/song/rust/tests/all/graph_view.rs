// graph_view.rs — the WebGraph a project draws: each step's flags and fields, each lane's clips, signals and strips, by JSON pointer.
// A reader would get wrong that the frontend reads presence: an absent key and a false one are different answers (437).

use crate::graph_support;

use graph_support::{Drawn, OUT_AMP, draw, no_null};
use serde_json::{Value, json};

fn at<'a>(d: &'a Drawn, ptr: &str) -> Option<&'a Value> {
    d.json.pointer(ptr)
}

fn clean(d: &Drawn) {
    let mut bad = Vec::new();
    no_null(&d.json, "", &mut bad);
    assert!(bad.is_empty(), "{bad:?}");
}

// graphFlags.test.ts "is no stub, and gets no engine address either": a shader, a shape and a transition ARE their behaviour.
#[test]
fn a_module_that_is_its_behaviour_is_no_stub_and_has_no_engine_address() {
    let d = draw(&[
        ("lib/k7-core/modules/shaders/sh/module.k7x", "type(\"shader\")\n"),
        ("nodes/sh.k7x", "extends(\"@k7-core/shaders/sh\")\nshader(\"blur\")\n"),
        ("graph.k7x", "modules({ sh: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/sh/use/0/stub"), Some(&json!(false)));
    assert_eq!(at(&d, "/chains/sh/use/0/pluginId"), None);
    assert_eq!(at(&d, "/chains/sh/use/0/hasNativePluginUI"), Some(&json!(false)));
    assert_eq!(at(&d, "/chains/sh/use/0/shader"), Some(&json!("blur")));
    assert_eq!(at(&d, "/chains/sh/use/0/category"), Some(&json!("shaders")));
    clean(&d);
}

// graphFlags.test.ts "leaves a real plugin slot with no plugin flagged"; a missing step carries `missing: true`.
#[test]
fn a_slot_with_no_plugin_is_a_stub_and_a_missing_module_says_so() {
    let d = draw(&[
        ("lib/k7-core/modules/audio/blank/module.k7x", "type(\"plugin\")\n"),
        ("nodes/blank.k7x", "extends(\"@k7-core/audio/blank\")\n"),
        ("graph.k7x", "modules({ blank: [\"ghost\"], ghost: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/blank/use/0/stub"), Some(&json!(true)));
    assert_eq!(at(&d, "/chains/blank/use/0/pluginId"), None);
    assert_eq!(at(&d, "/chains/blank/use/0/uses"), Some(&json!("@k7-core/audio/blank")));
    let missing = at(&d, "/chains/blank/use/1").expect("a step");
    assert_eq!(missing["missing"], json!(true));
    assert_eq!(missing["stub"], json!(false));
    assert_eq!(missing["hasNativePluginUI"], json!(false));
    assert_eq!(missing["params"], json!({}));
}

// offlineGraph.test.ts: no engine address, not unknown, declared params kept, a model to download only with a url.
#[test]
fn an_offline_module_has_no_address_and_a_missing_model_offers_its_url_only_with_one() {
    let offline = |url: &str| {
        format!(
            "plugin({{ type: \"offline\", name: \"Denoise\" }})\nparams({{ model: \"net.onnx\", strength: 0.5 }}){url}\n"
        )
    };
    let with = offline("\nmodel_url(\"https://example.com/net.onnx\")");
    let d = draw(&[
        ("nodes/n.k7x", with.as_str()),
        ("graph.k7x", "modules({ n: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/n/use/0/pluginId"), None);
    assert_eq!(at(&d, "/chains/n/use/0/unknownPlugin"), None);
    assert_eq!(at(&d, "/chains/n/use/0/params/strength"), Some(&json!(0.5)));
    assert_eq!(
        at(&d, "/chains/n/use/0/modelDownload"),
        Some(&json!({"file": "net.onnx", "url": "https://example.com/net.onnx"}))
    );
    let without = offline("");
    let d = draw(&[
        ("nodes/n.k7x", without.as_str()),
        ("graph.k7x", "modules({ n: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/n/use/0/modelDownload"), None);
    // Once the model is on the machine nothing is offered.
    let d = draw(&[
        ("nodes/n.k7x", with.as_str()),
        ("nodes/assets/net.onnx", "x"),
        ("graph.k7x", "modules({ n: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/n/use/0/modelDownload"), None);
}

// jsdspGraph.test.ts: a jsdsp module is not unknown, has its address and no native editor.
#[test]
fn a_jsdsp_module_has_its_address_and_no_native_editor() {
    let d = draw(&[
        ("nodes/j.k7x", "plugin({ type: \"jsdsp\" })\n"),
        ("graph.k7x", "modules({ j: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/j/use/0/unknownPlugin"), None);
    assert_eq!(at(&d, "/chains/j/use/0/pluginId"), Some(&json!("j:j")));
    assert_eq!(at(&d, "/chains/j/use/0/hasNativePluginUI"), Some(&json!(false)));
}

// 318: an external plugin is native when scanned, unknown when not, and not_supported is its own flag.
#[test]
fn an_external_plugin_is_unknown_or_native_by_the_scan_and_not_supported_is_its_own() {
    let scan = "{\"entries\":[{\"plugins\":[{\"name\":\"Real One\",\"format\":\"VST3\"}]}]}";
    let d = draw(&[
        ("scan.json", scan),
        ("nodes/real.k7x", "plugin({ type: \"VST3\", name: \"Real One\" })\n"),
        ("nodes/v.k7x", "plugin({ type: \"VST3\", name: \"Nothing Here\" })\n"),
        ("nodes/n.k7x", "plugin({ type: \"not_supported\", name: \"X\" })\n"),
        ("nodes/t.k7x", "plugin({ type: \"mystery\", name: \"X\" })\n"),
        (
            "graph.k7x",
            "modules({ v: [\"OUT\"], n: [\"OUT\"], t: [\"OUT\"], real: [\"OUT\"] })\n",
        ),
    ]);
    // A scanned plugin is native; an empty scan cannot say, so it never flags.
    assert_eq!(at(&d, "/chains/real/use/0/hasNativePluginUI"), Some(&json!(true)));
    assert_eq!(at(&d, "/chains/real/use/0/unknownPlugin"), None);
    assert_eq!(at(&d, "/chains/v/use/0/unknownPlugin"), Some(&json!(true)));
    assert_eq!(at(&d, "/chains/v/use/0/hasNativePluginUI"), Some(&json!(false)));
    assert_eq!(at(&d, "/chains/n/use/0/notSupported"), Some(&json!(true)));
    assert_eq!(at(&d, "/chains/n/use/0/unknownPlugin"), None);
    assert_eq!(at(&d, "/chains/t/use/0/unknownPlugin"), Some(&json!(true)));
    for c in ["v", "n", "t"] {
        assert_eq!(at(&d, &format!("/chains/{c}/use/0/pluginId")), None);
    }
}

// sfzKitGraph.test.ts: a kit's pads in note order, marked as a kit's; 442: a number mute only.
#[test]
fn an_sfz_kit_draws_its_pads_in_note_order_and_a_text_mute_is_not_one() {
    let kit = "plugin({ type: \"internal\", name: \"Sfz\" })\nkit({ snare: 38, kick: 36, hat: 42 })\nparams({ \"Mute 38\": 1, \"Mute 42\": \"1\" })\n";
    let d = draw(&[("nodes/k.k7x", kit), ("graph.k7x", "modules({ k: [\"OUT\"] })\n")]);
    let pads: Vec<(&str, i64, bool)> = at(&d, "/chains/k/use/0/rack")
        .and_then(Value::as_array)
        .expect("rack")
        .iter()
        .map(|p| {
            (
                p["symbol"].as_str().expect("s"),
                p["note"].as_i64().expect("n"),
                p.get("mute").is_some(),
            )
        })
        .collect();
    assert_eq!(pads, [("kick", 36, false), ("snare", 38, true), ("hat", 42, false)]);
    assert_eq!(at(&d, "/chains/k/use/0/rackKit"), Some(&json!(true)));
    clean(&d);
}

// A rack: sounds in note order, [] when empty, a slice rack's window and source, and no rackKit.
#[test]
fn a_rack_lists_its_sounds_and_an_empty_one_is_still_a_list() {
    let samples = "type(\"rack\")\nplugin({ type: \"internal\", name: \"Sampler\" })\nrack({ rack_type: \"samples\", symbols: { SN: { note: 38, clip: \"s\" }, BD: { note: 36, clip: \"b\", mute: true } } })\n";
    let slice = "type(\"rack\")\nplugin({ type: \"internal\", name: \"Sampler\" })\nrack({ rack_type: \"slice\", source: \"loop\", symbols: { A: { note: 60, from: 0, to: 0.5 } } })\n";
    let empty = "type(\"rack\")\nplugin({ type: \"internal\", name: \"Sampler\" })\n";
    let d = draw(&[
        ("nodes/s.k7x", samples),
        ("nodes/l.k7x", slice),
        ("nodes/e.k7x", empty),
        ("graph.k7x", "modules({ s: [\"OUT\"], l: [\"OUT\"], e: [\"OUT\"] })\n"),
    ]);
    let sounds = at(&d, "/chains/s/use/0/rack").and_then(Value::as_array).expect("rack");
    let sym: Vec<(&str, i64)> = sounds
        .iter()
        .map(|x| (x["symbol"].as_str().expect("s"), x["note"].as_i64().expect("n")))
        .collect();
    assert_eq!(sym, [("BD", 36), ("SN", 38)]);
    assert_eq!(sounds[0]["mute"], json!(true));
    assert!(
        sounds[1]["clip"].as_str().is_some_and(|c| c.ends_with("/s")),
        "{:?}",
        sounds[1]
    );
    let slice = at(&d, "/chains/l/use/0/rack").and_then(Value::as_array).expect("rack");
    assert!(
        slice[0]["source"].as_str().is_some_and(|c| c.ends_with("/loop")),
        "{:?}",
        slice[0]
    );
    assert_eq!(
        (slice[0]["from"].as_f64(), slice[0]["to"].as_f64()),
        (Some(0.0), Some(0.5))
    );
    assert!(slice[0].get("clip").is_none());
    assert_eq!(at(&d, "/chains/e/use/0/rack"), Some(&json!([])));
    assert_eq!(at(&d, "/chains/s/use/0/rackKit"), None);
    clean(&d);
}

// A camera and a MIDI device are PRESENT when empty; a label channel does not travel.
#[test]
fn a_camera_and_a_midi_device_are_present_empty_and_a_label_channel_does_not_travel() {
    let d = draw(&[
        ("nodes/cam.k7x", "io(\"camera\")\n"),
        ("nodes/dev.k7x", "io(\"midi\")\ndevice(\"Keys\")\n"),
        ("nodes/mic.k7x", "io(\"in\")\nchannel(\"synth\")\n"),
        (
            "graph.k7x",
            "modules({ cam: [\"RENDER\"], dev: [\"OUT\"], mic: [\"OUT\"] })\n",
        ),
    ]);
    assert_eq!(at(&d, "/chains/cam/use/0/camera"), Some(&json!("")));
    assert_eq!(at(&d, "/chains/dev/use/0/device"), Some(&json!("Keys")));
    assert_eq!(at(&d, "/chains/mic/use/0/channel"), None);
    assert_eq!(at(&d, "/chains/mic/use/0/monitor"), Some(&json!("auto")));
}

// 414 and 415: a step carries its base and an automation its trigger, and only automations whose curve exists.
#[test]
fn a_step_carries_its_base_and_an_automation_its_trigger_when_the_curve_exists() {
    let amp = format!(
        "{OUT_AMP}base(\"warm\")\nautomations([{{ param: \"cut\", curve: \"slow\", trigger: \"kick\" }}, {{ param: \"res\", curve: \"slow\" }}, {{ param: \"x\", curve: \"ghost\" }}])\n"
    );
    let d = draw(&[
        ("curves/slow.k7x", "type(\"lfo\")\nshape(\"sine\")\nfrequency(1)\n"),
        ("nodes/amp.k7x", amp.as_str()),
        ("graph.k7x", "modules({ amp: [\"OUT\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/amp/use/0/base"), Some(&json!("warm")));
    assert_eq!(
        at(&d, "/chains/amp/use/0/automations"),
        Some(&json!([{"param": "cut", "curve": "slow", "trigger": "kick"}, {"param": "res", "curve": "slow"}]))
    );
    assert_eq!(at(&d, "/curves/slow/type"), Some(&json!("lfo")));
    assert_eq!(at(&d, "/curves/slow/shape"), Some(&json!("sine")));
    assert!(at(&d, "/curves/slow/spec").is_some());
    clean(&d);
}

// 428: every curve that decodes travels, a broken one is absent with a finding and costs that curve alone.
#[test]
fn a_broken_curve_file_costs_that_curve_alone() {
    let d = draw(&[
        (
            "curves/ok.k7x",
            "type(\"onoff\")\npoints([{ time: \"1:1\", value: 1 }])\n",
        ),
        ("curves/bad.k7x", "type(\"lfo\")\nfrequency(\n"),
        ("graph.k7x", "modules({ a: [\"OUT\"] })\n"),
    ]);
    assert!(at(&d, "/curves/ok/points").is_some());
    assert!(at(&d, "/curves/bad").is_none());
    assert!(d.findings.iter().any(|f| f.file == "curves/bad.k7x"));
}

// The presence rules: inputs and outputs past one, mandatory only when false, events, stage, strip, alternatives.
#[test]
fn a_step_carries_ports_stage_events_and_alternatives_only_when_they_are_a_choice() {
    let two = "plugin({ type: \"internal\", name: \"X\" })\ninputs([{ name: \"a\", type: \"audio\" }, { name: \"b\", type: \"audio\", mandatory: false }])\noutputs([{ name: \"o\", type: \"audio\" }, { name: \"p\", type: \"midi\" }])\n";
    let synth = "plugin({ type: \"internal\", name: \"Synth\" })\n";
    let alt = "plugin({ type: \"internal\", name: \"Chorus\" })\nalternatives([{ name: \"wide\", params: { Depth: 1 } }])\nmix(0.4)\n";
    let d = draw(&[
        ("lib/k7-core/modules/instruments/synth/module.k7x", synth),
        ("lib/k7-core/modules/audio/two/module.k7x", two),
        ("lib/k7-core/modules/audio/alt/module.k7x", alt),
        ("nodes/synth.k7x", "extends(\"@k7-core/instruments/synth\")\n"),
        ("nodes/two.k7x", "extends(\"@k7-core/audio/two\")\n"),
        (
            "nodes/alt.k7x",
            "extends(\"@k7-core/audio/alt\")\nalternative(\"wide\")\n",
        ),
        (
            "graph.k7x",
            "modules({ synth: [\"OUT\"], two: [\"OUT\"], alt: [\"OUT\"] })\n",
        ),
    ]);
    assert_eq!(at(&d, "/chains/synth/use/0/events"), Some(&json!(["note-on"])));
    assert_eq!(at(&d, "/chains/synth/use/0/stage"), Some(&json!(["@output-volume"])));
    assert_eq!(
        at(&d, "/chains/two/use/0/inputs"),
        Some(&json!([{"name": "a", "type": "audio"}, {"name": "b", "type": "audio", "mandatory": false}]))
    );
    assert_eq!(
        at(&d, "/chains/two/use/0/outputs"),
        Some(&json!([{"name": "o", "type": "audio"}, {"name": "p", "type": "midi"}]))
    );
    assert_eq!(at(&d, "/chains/alt/use/0/alternatives"), Some(&json!(["1", "wide"])));
    assert_eq!(at(&d, "/chains/alt/use/0/alternative"), Some(&json!("wide")));
    assert_eq!(
        at(&d, "/chains/alt/use/0/stage"),
        Some(&json!(["@input-volume", "@mix", "@output-volume"]))
    );
    assert_eq!(at(&d, "/chains/alt/use/0/params/@mix"), Some(&json!(0.4)));
    assert_eq!(at(&d, "/chains/synth/use/0/inputs"), None);
    clean(&d);
}

// 2446: `missingModelOf` over a graph with no compile: no node, the model there, the model lacking with and without a url.
#[test]
fn a_nodes_missing_model_is_read_off_the_graph() {
    use graph_support::world;
    use song::graph::ModelNeed;
    use song::lanes::rig::Rig;
    let need = |url: &str, assets: &[(&str, &str)], node: &str| {
        let net =
            format!("plugin({{ type: \"offline\", name: \"Denoise\" }})\nparams({{ model: \"net.onnx\" }}){url}\n");
        let mut files = vec![
            ("nodes/n.k7x", net.as_str()),
            ("graph.k7x", "modules({ n: [\"OUT\"] })\n"),
        ];
        files.extend_from_slice(assets);
        let (src, roots) = world(&files);
        song::graph::model_need(
            &src,
            &mut song::cache::Cache::new(),
            &roots,
            Rig::from_json(&json!({})),
            node,
        )
        .unwrap_or_else(|e| panic!("graph refused: {e:?}"))
    };
    let url = "\nmodel_url(\"https://example.com/net.onnx\")";
    let wants = ModelNeed::Needs {
        file: "net.onnx".into(),
        url: Some("https://example.com/net.onnx".into()),
    };
    assert_eq!(need(url, &[], "n"), wants);
    assert_eq!(
        need("", &[], "n"),
        ModelNeed::Needs {
            file: "net.onnx".into(),
            url: None
        }
    );
    assert_eq!(need(url, &[("nodes/assets/net.onnx", "x")], "n"), ModelNeed::Have);
    assert_eq!(need(url, &[], "ghost"), ModelNeed::NoNode);
}
