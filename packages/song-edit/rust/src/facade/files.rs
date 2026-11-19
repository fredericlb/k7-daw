// files.rs — the project file writers: a lane's or a node's file, a module's clone, a live project, the graph text.
// A reader would get wrong `createNode`'s catalogue: the engine's modules are ambient in the TypeScript, so the host lists them in the call.

use super::args::{at, catalogue_of, done, opt, s, strs};
use super::{Ctx, Out};
use crate::chain_graph_edit as cge;
use crate::{project_files as pf, scaffold_live as sl};
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[
    (
        "song/projectFiles",
        &[
            "createTrackFile",
            "createPictureLaneFile",
            "createIoModuleFile",
            "freeNodeName",
            "createNode",
            "cloneModuleFile",
        ],
    ),
    (
        "song/scaffoldLive",
        &[
            "liveProjectFiles",
            "liveVideoProjectFiles",
            "scaffoldLive",
            "scaffoldLiveVideo",
        ],
    ),
    ("song/chainGraphEdit", &["chainGraphText", "projectGraphText"]),
];

const TRACK_REF: &str = "@meta/track";

fn files_json(f: &[(String, String)]) -> Json {
    Json::Object(f.iter().map(|(k, t)| (k.clone(), json!(t))).collect())
}

pub fn run(cx: &mut Ctx, _module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let env = cx.env;
    let dir = s(a, 0);
    Some(match f {
        "createTrackFile" => {
            let reference = opt(a, 2).unwrap_or_else(|| TRACK_REF.to_owned());
            done(
                crate::lane_files::create_track_file(fs, &dir, &s(a, 1), &reference, &mut *cx.new_id),
                |l| strs(&l),
            )
        }
        "createPictureLaneFile" => done(
            crate::lane_files::create_picture_lane_file(fs, env, &dir, &s(a, 1), &mut *cx.new_id),
            |l| strs(&l),
        ),
        "createIoModuleFile" => done(
            pf::create_io_module_file(fs, &dir, &s(a, 1), &s(a, 2), &mut *cx.new_id),
            |r| json!(r),
        ),
        "freeNodeName" => Ok(json!(pf::free_node_name(fs, env, &dir, &s(a, 1), opt(a, 2).as_deref()))),
        "createNode" => done(
            pf::create_node(
                fs,
                env,
                &dir,
                &s(a, 1),
                &s(a, 2),
                &s(a, 3),
                &catalogue_of(at(a, 4)),
                &mut *cx.new_id,
            ),
            |r| json!(r),
        ),
        "cloneModuleFile" => done(pf::clone_module_file(fs, &dir, &s(a, 1), &s(a, 2), &s(a, 3)), |r| {
            json!(r)
        }),
        "liveProjectFiles" => Ok(files_json(&sl::live_project_files(&dir))),
        "liveVideoProjectFiles" => Ok(files_json(&sl::live_video_project_files(&dir))),
        "scaffoldLive" => {
            let name = s(a, 1);
            if opt(a, 2).as_deref() == Some("graph") {
                done(sl::scaffold_live_graph(fs, &dir, &name), |l| strs(&l))
            } else {
                done(sl::scaffold_live(fs, &dir, &name, &mut *cx.new_id), |l| strs(&l))
            }
        }
        "scaffoldLiveVideo" => done(sl::scaffold_live_video(fs, &dir, &s(a, 1), &mut *cx.new_id), |l| {
            strs(&l)
        }),
        "chainGraphText" => done(cge::chain_graph_text(fs, &dir), |t| json!(t)),
        "projectGraphText" => done(cge::project_graph_text(fs, &dir), |t| json!(t)),
        _ => return None,
    })
}
