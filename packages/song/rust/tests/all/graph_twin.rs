// graph_twin.rs — writes the TypeScript declarations ts-rs derives for the WebGraph types, for scripts/graphTwinCheck.ts to hold against the hand-written ones.
//
//   K7_SONG_TWIN=<dir> cargo test --features ts --test all -- --ignored graph_twin::      then      npx tsx scripts/graphTwinCheck.ts <dir>
//
// What a reader gets wrong: the check runs from the generated type to the hand-written one (what the route gives is what the frontend reads),
// and `no_null` holds what no type can: a null, or a float in an integer field (decision 438).

use crate::graph_support;

use graph_support::no_null;
use serde_json::json;
use song::graph::view::web::{
    WebAltChoice, WebAmpModel, WebAssemblyParam, WebAutomation, WebChain, WebChainStep, WebCurve, WebDocNote, WebGraph,
    WebModelDownload, WebNodeAssembly, WebOut, WebPlug, WebPortIn, WebPortOut, WebRackSound, WebTap, WebTrackClip,
};
use song::graph::{GraphIssue, WarningCode};
use song::modules::PortType;
use ts_rs::TS;

#[test]
#[ignore = "writes the declarations for scripts/graphTwinCheck.ts: K7_SONG_TWIN"]
fn the_declarations_are_written_for_the_type_check() {
    let dir = std::path::PathBuf::from(std::env::var("K7_SONG_TWIN").expect("K7_SONG_TWIN"));
    std::fs::create_dir_all(&dir).expect("the folder");
    let decls = [
        PortType::decl(),
        WarningCode::decl(),
        GraphIssue::decl(),
        WebOut::decl(),
        WebPortIn::decl(),
        WebPortOut::decl(),
        WebModelDownload::decl(),
        WebAmpModel::decl(),
        WebRackSound::decl(),
        WebAutomation::decl(),
        WebTap::decl(),
        WebAltChoice::decl(),
        WebDocNote::decl(),
        WebTrackClip::decl(),
        WebPlug::decl(),
        WebAssemblyParam::decl(),
        WebNodeAssembly::decl(),
        WebCurve::decl(),
        WebChainStep::decl(),
        WebChain::decl(),
        WebGraph::decl(),
    ];
    let text: Vec<String> = decls.iter().map(|d| format!("export {d}")).collect();
    std::fs::write(dir.join("twin.ts"), text.join("\n")).expect("the declarations");
}

// 438 (3): no null and no float in an integer field, over a graph that holds every kind of field.
#[test]
fn no_null_holds_over_a_graph_with_ports_racks_and_an_amp() {
    let amp = "plugin({ type: \"internal\", name: \"Amp\" })\nparams({ model: \"n.nam\" })\n";
    let d = graph_support::draw(&[
        ("nodes/amp.k7x", amp),
        ("nodes/assets/n.nam", "x"),
        ("nodes/in.k7x", "io(\"in\")\nchannel([1, 2])\n"),
        ("graph.k7x", "modules({ amp: [\"OUT\"], in: [\"OUT\"] })\n"),
    ]);
    let mut bad = Vec::new();
    no_null(&d.json, "", &mut bad);
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(d.json.pointer("/chains/in/use/0/channel"), Some(&json!([1, 2])));
}
