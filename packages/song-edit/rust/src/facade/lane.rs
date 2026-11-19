// lane.rs — the lane and module side's arms: the base preset, chains, the timelines, the scaffolds, the module fork.
// A reader would get wrong `scaffoldSynth`: it writes into an empty memory folder and answers the file's text, not the caller's folder.

use super::args::{at, catalogue_of, done, fnum, opt_json, s, val_lax};
use super::{Ctx, Out};
use crate::{
    Fs, MemFs, base_preset, chain_create, chain_stack, module_fork, scaffold, timeline_from_range,
    timeline_rename,
};
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[
    ("song/basePreset", &["presetFor", "basePresetIn", "isSurgePatch"]),
    ("song-edit/timelineRename", &["renameTimeline"]),
    (
        "song/chainCreate",
        &["chainTaken", "createChainLane", "createChainNode", "insertChainInput"],
    ),
    ("song/chainStack", &["stackChainNode"]),
    ("song-edit/timelineFromRange", &["timelineFromRange"]),
    ("song-edit/trackMix", &["timelineFromRange"]),
    (
        "song/scaffold",
        &[
            "writeProjectFiles",
            "scaffoldSynth",
            "scaffold",
        ],
    ),
    (
        "song/moduleFork",
        &[
            "materialiseEngineModule",
            "sharedRoots",
            "sharedModuleRef",
            "chainsUsing",
        ],
    ),
];

pub fn run(cx: &mut Ctx, module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let env = cx.env;
    Some(match (module, f) {
        ("song/basePreset", "presetFor") => Ok(opt_json(base_preset::preset_for(
            fs,
            &s(a, 0),
            &s(a, 1),
            at(a, 2).as_str(),
            &val_lax(at(a, 3)),
        ))),
        ("song/basePreset", "basePresetIn") => {
            Ok(opt_json(base_preset::base_preset_in(fs, &s(a, 0), &val_lax(at(a, 1)))))
        }
        ("song/basePreset", "isSurgePatch") => Ok(json!(base_preset::is_surge_patch(a.first().and_then(Json::as_str)))),
        ("song-edit/timelineRename", "renameTimeline") => done(
            timeline_rename::rename_timeline(fs, &s(a, 0), &s(a, 1), &s(a, 2)),
            |n| json!(n),
        ),
        ("song/chainCreate", "chainTaken") => Ok(json!(chain_create::chain_taken(fs, &s(a, 0), &s(a, 1)))),
        ("song/chainCreate", "createChainLane") => done(
            chain_create::create_chain_lane(fs, &s(a, 0), &s(a, 1), &s(a, 2), &mut *cx.new_id),
            |v| json!(v),
        ),
        ("song/chainCreate", "createChainNode") => done(
            chain_create::create_chain_node(fs, &s(a, 0), &s(a, 1), &s(a, 2), &mut *cx.new_id),
            |(id, rel)| json!({ "id": id, "rel": rel }),
        ),
        ("song/chainCreate", "insertChainInput") => done(
            chain_create::insert_chain_input(fs, &s(a, 0), &s(a, 1), &s(a, 2), &mut *cx.new_id),
            |(id, rel)| json!({ "id": id, "rel": rel }),
        ),
        ("song/chainStack", "stackChainNode") => {
            done(chain_stack::stack_chain_node(fs, &s(a, 0), &s(a, 1), &s(a, 2)), |l| {
                json!(l)
            })
        }
        ("song-edit/timelineFromRange" | "song-edit/trackMix", "timelineFromRange") => done(
            timeline_from_range::timeline_from_range(
                fs,
                &s(a, 0),
                &s(a, 1),
                fnum(a, 2),
                fnum(a, 3),
                fnum(a, 4),
                fnum(a, 5),
                &s(a, 6),
            ),
            |n| json!(n),
        ),
        ("song/moduleFork", "materialiseEngineModule") => {
            let cat = catalogue_of(at(a, 2));
            done(
                module_fork::materialise_engine_module(fs, env, &s(a, 0), &s(a, 1), &cat, None),
                |f| json!({ "ref": f.reference, "module": f.module, "path": f.path, "chains": f.chains }),
            )
        }
        ("song/moduleFork", "sharedRoots") => Ok(Json::Array(
            module_fork::shared_roots(fs, env, &s(a, 0))
                .into_iter()
                .map(|(r, n)| json!({ "root": r, "ns": n }))
                .collect(),
        )),
        ("song/moduleFork", "sharedModuleRef") => Ok(module_fork::shared_module_ref(fs, env, &s(a, 0), &s(a, 1))
            .map_or(
                Json::Null,
                |r| json!({ "ref": r.reference, "folder": r.folder, "leaf": r.leaf, "type": r.kind }),
            )),
        ("song/moduleFork", "chainsUsing") => Ok(json!(module_fork::chains_using(&s(a, 0), &s(a, 1)))),
        ("song/scaffold", "writeProjectFiles") => {
            let files: Vec<(String, String)> = at(a, 1)
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                        .collect()
                })
                .unwrap_or_default();
            done(scaffold::write_project_files(fs, &s(a, 0), &files), |k| json!(k))
        }
        ("song/scaffold", "scaffoldSynth") => {
            let dir = s(a, 0);
            let mut empty = MemFs::new();
            let _ = Fs::mkdir_all(&mut empty, &dir);
            done(scaffold::ensure_workspace_synth(&mut empty, &dir), |()| {
                let path = format!("{dir}/../_modules/instruments/synth/module.k7x");
                json!(String::from_utf8_lossy(
                    empty
                        .files()
                        .find(|(p, _)| *p == crate::paths::resolve(&path))
                        .map_or(&[][..], |(_, b)| b)
                ))
            })
        }
        ("song/scaffold", "scaffold") => {
            let shape = if s(a, 2) == "video" {
                scaffold::TimelineShape::Video
            } else {
                scaffold::TimelineShape::Arrangement
            };
            // The video shape copies the sample the host names in the fourth argument.
            let sample = at(a, 3).as_str().map(str::as_bytes);
            let out = scaffold::scaffold(fs, &s(a, 0), &s(a, 1), shape, sample, &mut *cx.new_id);
            done(out, |l| json!(l))
        }
        _ => return None,
    })
}
