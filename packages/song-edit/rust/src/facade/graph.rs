// graph.rs — the node side's graph arms: bypass, wiring, positions, the tap chain, the lanes' files, the presets.
// A reader would get wrong the ids: a lane or a tap node draws its ids from the host's `new_id`, in the order the TypeScript drew them.

use super::args::{at, b, done, json_of, n, opt, params, s, strs, undefined};
use super::node::pos;
use super::{Ctx, Out};
use crate::graph_edit as ge;
use crate::lane_files as lf;
use crate::node_presets as np;
use crate::tap_chain as tc;
use serde_json::{Value as Json, json};

const GRAPH: &[&str] = &[
    "readGraphValue",
    "setModuleBypass",
    "setLaneBypass",
    "setBypassEntry",
    "unlinkChainNode",
    "laneOfNode",
    "renameChainNode",
    "deleteChainNode",
    "placeChainNode",
    "setChainNodePositions",
    "clearChainNodePositions",
    "setChainNodePosition",
    "connectChainNodes",
    "disconnectChainNodes",
    "replugChainNodes",
    "insertChainStep",
];

pub const NAMES: &[(&str, &[&str])] = &[
    ("song-edit/projectEdit", GRAPH),
    ("song-edit/graphEdit", GRAPH),
    (
        "song-edit/tapChain",
        &["addTapChainNode", "removeTapChainNode", "moveTapChainNode"],
    ),
    ("song/laneStrip", &["stripNewLane", "createPictureLane"]),
    (
        "song-edit/presets",
        &[
            "presetFile",
            "presetRefOf",
            "listNodePresets",
            "writeNodePreset",
            "applyPresetToNode",
        ],
    ),
];

fn preset_json(p: &crate::presets::PresetFile) -> Json {
    let mut o = serde_json::Map::new();
    if let Some(x) = &p.engine {
        o.insert("engine".to_owned(), json!(x));
    }
    if let Some(x) = &p.base {
        o.insert("base".to_owned(), json!(x));
    }
    if let Some(x) = &p.params {
        o.insert("params".to_owned(), params(x));
    }
    if let Some(r) = &p.rack {
        let kit = r.iter().map(|(k, t)| {
            let mut tap = json!({ "note": t.note });
            if let Some(s) = &t.sample {
                tap["sample"] = json!(s);
            }
            (k.clone(), tap)
        });
        o.insert("rack".to_owned(), Json::Object(kit.collect()));
    }
    Json::Object(o)
}

pub fn run(cx: &mut Ctx, _module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let e = cx.env;
    let dir = s(a, 0);
    let m = s(a, 1);
    let ok_path = |r: Result<String, crate::Error>| done(r, |p| json!(p));
    Some(match f {
        "readGraphValue" => done(ge::read_graph_value(fs, &dir), |v| json_of(&v)),
        "setModuleBypass" => ok_path(ge::set_module_bypass(fs, e, &dir, &m, b(a, 2))),
        "setLaneBypass" => ok_path(ge::set_lane_bypass(fs, e, &dir, &m, b(a, 2))),
        "setBypassEntry" => ok_path(ge::set_bypass_entry(fs, e, &dir, &m, b(a, 2))),
        "unlinkChainNode" => done(
            ge::unlink_chain_node(fs, e, &dir, &m),
            |r| json!({ "rewired": strs(&r) }),
        ),
        "laneOfNode" => Ok(ge::lane_of_node(fs, &dir, &m).map_or(Json::Null, |l| json!(l))),
        "renameChainNode" => done(
            ge::rename_chain_node(fs, e, &dir, &m, &s(a, 2)),
            |(mv, rw)| json!({ "moved": strs(&mv), "rewired": strs(&rw) }),
        ),
        "deleteChainNode" => done(
            ge::delete_chain_node(fs, e, &dir, &m),
            |(rw, rm)| json!({ "rewired": strs(&rw), "removed": strs(&rm) }),
        ),
        "placeChainNode" => ok_path(ge::place_chain_node(fs, e, &dir, &m)),
        "setChainNodePositions" => {
            let at_: Vec<(String, (f64, f64))> = at(a, 1)
                .as_object()
                .map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), pos(v)?))).collect())
                .unwrap_or_default();
            ok_path(ge::set_chain_node_positions(fs, e, &dir, &at_))
        }
        "clearChainNodePositions" => ok_path(ge::clear_chain_node_positions(fs, e, &dir)),
        "setChainNodePosition" => ok_path(ge::set_chain_node_position(fs, e, &dir, &m, a.get(2).and_then(pos))),
        "connectChainNodes" => ok_path(ge::connect_chain_nodes(fs, e, &dir, &m, &s(a, 2))),
        "disconnectChainNodes" => ok_path(ge::disconnect_chain_nodes(fs, e, &dir, &m, &s(a, 2))),
        "replugChainNodes" => ok_path(ge::replug_chain_nodes(fs, e, &dir, [&m, &s(a, 2), &s(a, 3), &s(a, 4)])),
        "insertChainStep" => ok_path(ge::insert_chain_step(fs, e, &dir, &m, opt(a, 2).as_deref(), &s(a, 3))),
        "addTapChainNode" => done(
            tc::add_tap_chain_node(fs, e, &dir, &m, &s(a, 2), &s(a, 3), &mut *cx.new_id),
            |(sub, id)| json!({ "subchain": sub, "id": id }),
        ),
        "removeTapChainNode" => done(
            tc::remove_tap_chain_node(fs, e, &dir, &m, &s(a, 2), &s(a, 3)),
            |emptied| json!({ "emptied": emptied }),
        ),
        "moveTapChainNode" => done(
            tc::move_tap_chain_node(fs, e, &dir, &m, &s(a, 2), &s(a, 3), n(a, 4) as i64),
            |()| undefined(),
        ),
        "stripNewLane" => {
            let (written, no_strip) = crate::lane_strip_plan::strip_new_lane(fs, e, &dir, &m, &mut *cx.new_id);
            Ok(json!({ "written": written, "noStrip": no_strip }))
        }
        "createPictureLane" => done(
            lf::create_picture_lane(fs, e, &dir, &m, &mut *cx.new_id),
            |(written, no_strip)| json!({ "written": written, "noStrip": no_strip }),
        ),
        "presetFile" => ok_path(crate::presets::preset_file(fs, e, &dir, &m, &s(a, 2), &s(a, 3))),
        "presetRefOf" => done(crate::presets::preset_ref_of(fs, e, &dir, &m), |r| {
            r.map_or(Json::Null, |x| json!(x))
        }),
        "listNodePresets" => done(crate::presets::list_node_presets(fs, e, &dir, &m), |l| {
            let presets: Vec<Json> = l
                .presets
                .iter()
                .map(|p| json!({ "scope": p.scope, "name": p.name }))
                .collect();
            let reference = l.reference.map_or(Json::Null, |x| json!(x));
            json!({ "ref": reference, "presets": presets, "scopes": l.scopes })
        }),
        "writeNodePreset" => {
            let mode = if s(a, 4) == "update" {
                np::PresetMode::Update
            } else {
                np::PresetMode::New
            };
            done(
                np::write_node_preset(fs, e, &dir, &m, &s(a, 2), &s(a, 3), mode),
                |(path, r, p)| json!({ "path": path, "ref": r, "preset": preset_json(&p) }),
            )
        }
        "applyPresetToNode" => done(np::apply_preset_to_node(fs, e, &dir, &m, &s(a, 2), &s(a, 3)), |r| {
            let mut o =
                json!({ "params": params(&r.params), "switched": r.switched, "rebased": r.rebased, "kit": r.kit });
            if let Some(x) = r.missing_engine {
                o["missingEngine"] = json!(x);
            }
            if let Some(x) = r.missing_base {
                o["missingBase"] = json!(x);
            }
            o
        }),
        _ => return None,
    })
}
