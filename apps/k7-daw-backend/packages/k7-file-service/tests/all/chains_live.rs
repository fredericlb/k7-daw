//! The live list: why a chain cannot be played on the rig (`refusal`), sentence for sentence.

use k7_file_service::chains::live_chains;
use serde_json::json;

use crate::support::chains::World;

const COMP: &str = "plugin({ type: \"internal\", name: \"Compressor\" })\n";
const GATE: &str = "cold(true)\nplugin({ type: \"internal\", name: \"Gate\" })\n";

fn chain(steps: &str) -> String {
    format!("steps([{steps}, to([chain(\"@out\")])])\n")
}

#[test]
fn a_cold_flagged_live_module_needs_the_disk_and_the_other_refusals_read_as_written() {
    let ok = chain("id(\"a\").extends(\"@k7-core/audio/comp\")");
    let gated = chain("id(\"g\").extends(\"@k7-core/audio/gate\")");
    let w = World::new(
        "chains-live",
        &[
            ("libraries/k7-core/modules/audio/comp/module.k7x", COMP),
            ("libraries/k7-core/modules/audio/gate/module.k7x", GATE),
            ("studio/library/chains/ok.k7x", &ok),
            ("studio/library/chains/gated.k7x", &gated),
            (
                "studio/library/chains/empty.k7x",
                "steps([to([chain(\"@out\")])])\n",
            ),
            (
                "libraries/k7-core/chains/fold/graph.k7x",
                "modules({ \"@in\": [\"amp\"], amp: [\"gone\"], gone: [\"@out\"] })\n",
            ),
            (
                "libraries/k7-core/chains/fold/nodes/amp.k7x",
                "extends(\"@k7-core/audio/comp\")\n",
            ),
        ],
    );
    let got = live_chains(&w.chains()).expect("a list");
    assert_eq!(
        got,
        json!({ "chains": [
            { "ref": "@account/empty", "unavailable": "it holds no step" },
            { "ref": "@account/gated", "unavailable": "\"@k7-core/audio/gate\" only works on audio already on disk" },
            { "ref": "@account/ok", "unavailable": null },
            { "ref": "@k7-core/fold", "unavailable": "it lacks gone (no nodes/gone.k7x)" },
        ] })
    );
}

#[test]
fn a_folder_chains_node_named_like_the_rigs_is_refused() {
    let w = World::new(
        "chains-live-rig",
        &[
            ("libraries/k7-core/modules/audio/comp/module.k7x", COMP),
            (
                "libraries/k7-core/chains/fold/graph.k7x",
                "modules({ \"@in\": [\"fader\"], fader: [\"@out\"] })\n",
            ),
            (
                "libraries/k7-core/chains/fold/nodes/fader.k7x",
                "extends(\"@k7-core/audio/comp\")\n",
            ),
        ],
    );
    let got = live_chains(&w.chains()).expect("a list");
    assert_eq!(
        got["chains"][0]["unavailable"],
        "its node \"fader\" has the name of one of the rig's own"
    );
}
