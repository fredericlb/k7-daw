//! The cold list and the settings' view: a chain is cold when every step is offline, and a missing node is a step (1p-D24).

use k7_file_service::chains::{ChainSettings, cold_chains, cold_chains_view};
use serde_json::json;

use crate::support::chains::World;

const VOX: &str = "cold(true)\nplugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\nmodel_url(\"https://m.example/v.onnx\")\noutputs([name(\"instrumental\").type(\"audio\")])\n";

#[test]
fn a_folder_chain_with_a_missing_node_is_never_cold_though_its_steps_that_read_are() {
    let w = World::new(
        "chains-cold",
        &[
            ("libraries/k7-core/modules/cold/vox/module.k7x", VOX),
            (
                "libraries/k7-core/chains/ok.k7x",
                "steps([id(\"s\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n",
            ),
            (
                "libraries/k7-core/chains/fold/graph.k7x",
                "modules({ \"@in\": [\"sp\", \"gone\"], sp: [\"gone\"], gone: [\"@out\"] })\n",
            ),
            (
                "libraries/k7-core/chains/fold/nodes/sp.k7x",
                "extends(\"@k7-core/cold/vox\")\n",
            ),
        ],
    );
    let got = cold_chains(&w.chains()).expect("a list");
    assert_eq!(
        got,
        json!([{ "ref": "@k7-core/ok", "instrumental": true,
            "unavailable": "step '@k7-core/cold/vox' wants the model 'vox.onnx', which is not on this machine — put it in ~/.k7/models (or the module's own assets/)",
            "downloads": [{ "file": "vox.onnx", "url": "https://m.example/v.onnx" }] }])
    );
}

#[test]
fn the_view_carries_the_two_settings_and_null_for_one_not_set() {
    let w = World::new(
        "chains-cold-view",
        &[("libraries/k7-core/modules/cold/vox/module.k7x", VOX)],
    );
    let prefs = ChainSettings {
        stems: Some("@k7-core/ok".into()),
        instrumental: None,
    };
    let view = cold_chains_view(&w.chains(), &prefs).expect("a view");
    assert_eq!(
        view,
        json!({ "chains": [], "stemsChain": "@k7-core/ok", "instrumentalChain": null })
    );
}
