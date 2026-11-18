// node.rs — the node side's arms: a module's parameters and alternatives, the clips' onsets and warp, the rack's taps, the assemblies.
// A reader would get wrong that the workspace's own modules are not found here: the `Fs` the host gives must hold what the loader reads.

use super::args::{at, done, json_of, n, opt, params, s, undefined, val_strict};
use super::{Ctx, Failure, Out};
use crate::assembly_edit as ae;
use crate::chain_graph_edit as cge;
use crate::rack_taps as rt;
use crate::{alternative_switch as alt, clip_edit as clip, engine_switch as eng, project_edit as pe};
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[
    (
        "song-edit/projectEdit",
        &[
            "setModuleParam",
            "setModuleAsset",
            "setModuleFrozen",
            "setModuleMonitor",
            "setModuleShader",
            "setModuleCamera",
            "setModuleDevice",
            "setModuleBase",
            "setModuleInputGain",
            "setModuleMix",
            "setModuleVolume",
            "setModuleChannel",
            "setModuleAutomationBinding",
            "setModuleExtends",
            "moduleAlternatives",
            "newModuleAlternative",
            "clipFilePath",
            "clipOnsets",
            "clipWarpMarkers",
            "setClipOnsets",
            "setClipWarp",
        ],
    ),
    ("song-edit/alternativeSwitch", &["selectModuleAlternative"]),
    ("song-edit/engineSwitch", &["moduleEngines", "selectModuleEngine"]),
    (
        "song-edit/rackTaps",
        &[
            "kitOf",
            "tapsOf",
            "setTap",
            "addTap",
            "renameTap",
            "removeTap",
            "setTapOut",
            "setKit",
        ],
    ),
    (
        "song/assemblyEdit",
        &[
            "createAssembly",
            "assemblyMembers",
            "dissolveAssembly",
            "renameNodeInAssemblies",
            "dropNodeFromAssemblies",
        ],
    ),
    ("song/chainGraphEdit", &["chainNodeName"]),
];

pub fn pos(j: &Json) -> Option<(f64, f64)> {
    Some((j.get("x")?.as_f64()?, j.get("y")?.as_f64()?))
}

pub fn run(cx: &mut Ctx, _module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let e = cx.env;
    let dir = s(a, 0);
    let m = s(a, 1);
    let ok_path = |r: Result<String, crate::Error>| done(r, |p| json!(p));
    Some(match f {
        "setModuleParam" => ok_path(pe::set_module_param(fs, e, &dir, &m, &s(a, 2), n(a, 3))),
        "setModuleAsset" => ok_path(pe::set_module_asset(fs, e, &dir, &m, &s(a, 2), opt(a, 3).as_deref())),
        "setModuleFrozen" => ok_path(pe::set_module_frozen(fs, e, &dir, &m, opt(a, 2).as_deref())),
        "setModuleMonitor" => ok_path(pe::set_module_monitor(fs, e, &dir, &m, &s(a, 2))),
        "setModuleShader" => ok_path(pe::set_module_shader(fs, e, &dir, &m, &s(a, 2))),
        "setModuleCamera" => ok_path(pe::set_module_camera(fs, e, &dir, &m, &s(a, 2))),
        "setModuleDevice" => ok_path(pe::set_module_device(fs, e, &dir, &m, &s(a, 2))),
        "setModuleBase" => ok_path(pe::set_module_base(fs, e, &dir, &m, &s(a, 2))),
        "setModuleInputGain" => ok_path(pe::set_module_input_gain(fs, e, &dir, &m, n(a, 2))),
        "setModuleMix" => ok_path(pe::set_module_mix(fs, e, &dir, &m, n(a, 2))),
        "setModuleVolume" => ok_path(pe::set_module_volume(fs, e, &dir, &m, &s(a, 2), n(a, 3))),
        "setModuleChannel" => ok_path(pe::set_module_channel(fs, e, &dir, &m, &floats_nan(at(a, 2)))),
        "setModuleAutomationBinding" => ok_path(pe::set_module_automation_binding(
            fs,
            e,
            &dir,
            &m,
            &s(a, 2),
            opt(a, 3).as_deref(),
            opt(a, 4).as_deref(),
        )),
        "setModuleExtends" => done(
            pe::set_module_extends(fs, e, &dir, &m, &s(a, 2)),
            |(rel, dropped, alt)| {
                let mut o = json!({ "rel": rel, "dropped": dropped });
                if let Some(x) = alt {
                    o["alternative"] = json!(x);
                }
                o
            },
        ),
        "moduleAlternatives" => done(
            pe::module_alternatives(fs, e, &dir, &m),
            |(names, current)| json!({ "names": names, "current": current }),
        ),
        "newModuleAlternative" => {
            let copy = at(a, 3).get("copy").and_then(Json::as_bool).unwrap_or(false);
            done(
                pe::new_module_alternative(fs, e, &dir, &m, opt(a, 2).as_deref(), copy),
                |(path, name)| json!({ "path": path, "name": name }),
            )
        }
        "selectModuleAlternative" => done(
            alt::select_module_alternative(fs, e, &dir, &m, &s(a, 2)),
            |(path, values)| {
                let mut o = json!({ "path": path });
                if let Some(v) = values {
                    o["values"] = json!({ "params": params(&v.params), "whole": v.whole });
                }
                o
            },
        ),
        "moduleEngines" => done(eng::module_engines(fs, e, &dir, &m), |(names, cur)| {
            let mut o = json!({ "names": names });
            if let Some(c) = cur {
                o["current"] = json!(c);
            }
            o
        }),
        "selectModuleEngine" => done(
            eng::select_module_engine(fs, e, &dir, &m, &s(a, 2)),
            |p| json!({ "path": p }),
        ),
        "clipFilePath" => done(clip::clip_file_path(fs, &dir, &m), |p| json!(p)),
        "clipOnsets" => Ok(json_of(&clip::clip_onsets(fs, &dir, &m))),
        "clipWarpMarkers" => done(clip::clip_warp_markers(fs, &dir, &m), |r| {
            r.map_or(Json::Null, |v| {
                Json::Array(v.iter().map(|(x, y)| json!([x, y])).collect())
            })
        }),
        "setClipOnsets" => match val_strict(at(a, 2)) {
            Some(edits) => ok_path(clip::set_clip_onsets(fs, &dir, &m, &edits)),
            None => Err(Failure::Unsupported("an argument JSON cannot carry")),
        },
        "setClipWarp" => {
            let pt = |p: &Json, i: usize| p.get(i).and_then(Json::as_f64);
            let mk: Vec<[Option<f64>; 2]> = at(a, 2)
                .as_array()
                .map(|l| l.iter().map(|p| [pt(p, 0), pt(p, 1)]).collect())
                .unwrap_or_default();
            let alg = opt(a, 3).unwrap_or_else(|| "elastique".to_owned());
            ok_path(clip::set_clip_warp(fs, &dir, &m, &mk, &alg))
        }
        "kitOf" => done(rt::kit_of(fs, e, &dir, &m), |r| {
            let mut o = json!({ "rackType": if r.rack_type == song::modules::RackType::Slice { "slice" } else { "samples" },
                "symbols": serde_json::to_value(&r.symbols).unwrap_or(Json::Null) });
            o["source"] = r.source.as_ref().map_or_else(undefined, |x| json!(x));
            // The resolver leaves `clip` and `source` as explicit undefined, which the harvest records.
            if let Some(Json::Object(syms)) = o.get_mut("symbols") {
                for v in syms.values_mut() {
                    if v.get("clip").is_none() {
                        v["clip"] = undefined();
                    }
                }
            }
            o
        }),
        "tapsOf" => done(rt::taps_of(fs, e, &dir, &m), |t| json!(t)),
        "setTap" => {
            let c = at(a, 3);
            let change = rt::TapChange {
                note: c.get("note").and_then(Json::as_f64),
                sample: match c.get("sample") {
                    Some(Json::Null) => Some(None),
                    Some(Json::String(x)) => Some(Some(x.clone())),
                    _ => None,
                },
                mute: c.get("mute").and_then(Json::as_bool),
            };
            ok_path(rt::set_tap(fs, e, &dir, &m, &s(a, 2), &change))
        }
        "addTap" => {
            let near = match a.get(3) {
                Some(Json::Number(x)) => x.as_f64().map(rt::TapNear::Note),
                Some(o @ Json::Object(_)) if o.get("symbol").is_some() => Some(rt::TapNear::Beside {
                    symbol: o["symbol"].as_str().unwrap_or("").to_owned(),
                    before: o["side"] == "before",
                }),
                _ => None,
            };
            done(
                rt::add_tap(fs, e, &dir, &m, opt(a, 2).as_deref(), near.as_ref()),
                |(symbol, note)| json!({ "symbol": symbol, "note": note }),
            )
        }
        "renameTap" => ok_path(rt::rename_tap(fs, e, &dir, &m, &s(a, 2), &s(a, 3))),
        "removeTap" => ok_path(rt::remove_tap(fs, e, &dir, &m, &s(a, 2))),
        "setTapOut" => ok_path(rt::set_tap_out(fs, e, &dir, &m, &s(a, 2), opt(a, 3).as_deref())),
        "setKit" => {
            let taps: Vec<(String, f64, Option<String>)> = at(a, 2)
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                v["note"].as_f64().unwrap_or(f64::NAN),
                                v["sample"].as_str().map(str::to_owned),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            ok_path(rt::set_kit(fs, e, &dir, &m, &taps))
        }
        "createAssembly" => done(
            ae::create_assembly(fs, e, &dir, &m, &s(a, 2), a.get(3).and_then(pos)),
            |(file, members)| json!({ "file": file, "members": Json::Object(members.into_iter().map(|(k, v)| (k, json!(v))).collect()) }),
        ),
        "assemblyMembers" => done(ae::assembly_members(fs, &dir, &m), |v| json!(v)),
        "dissolveAssembly" => ok_path(ae::dissolve_assembly(fs, &dir, &m)),
        "renameNodeInAssemblies" => done(ae::rename_node_in_assemblies(fs, &dir, &m, &s(a, 2)), |v| json!(v)),
        "dropNodeFromAssemblies" => done(ae::drop_node_from_assemblies(fs, &dir, &m), |v| json!(v)),
        "chainNodeName" => done(cge::chain_node_name(fs, &dir, &m), |v| json!(v)),
        _ => return None,
    })
}

/// A list of numbers where a missing one is NaN, as `Number(x)` would read it.
fn floats_nan(j: &Json) -> Vec<f64> {
    j.as_array()
        .map(|l| l.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}
