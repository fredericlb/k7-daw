// base.rs — the base's arms: the yaml and workspace file writes, the module folder reads, the confinement and the chain item reads.
// A reader would get wrong that the name table is the contract: an arm no entry lists is never reached, and a test names it.

use super::args::{at, done, json_of, path_of, s, seg_json, string_list, undefined, val_strict};
use super::{Ctx, Failure, Out};
use crate::chain_item_edit as cie;
use crate::confine;
use crate::module_io::{self as mio, ModuleFile};
use crate::workspace_doc as wd;
use crate::workspace_file as wf;
use crate::{format_stamp, node_files, yaml_edit};
use k7x::edit::StringifyOptions;
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[
    ("song/yaml/edit", &["setFileValue", "deleteFileValue"]),
    (
        "song/yaml/workspaceDoc",
        &["wsText", "wsData", "wsWriteText", "wsWriteData"],
    ),
    (
        "song/yaml/workspaceFile",
        &[
            "wsExists",
            "isDocName",
            "wsRead",
            "wsWrite",
            "wsRemove",
            "wsCopy",
            "k7xData",
        ],
    ),
    (
        "song/yaml/moduleIO",
        &[
            "entityFileFor",
            "moduleFileFor",
            "entityFiles",
            "nodeFiles",
            "isProjectFolder",
            "readModuleFile",
        ],
    ),
    ("song/formatStamp", &["stampFormatLevel"]),
    (
        "song/confine",
        &["confineTo", "projectRoots", "workspaceDirOf", "confineToProject"],
    ),
    ("song/nodeFiles", &["copyNodeFiles"]),
    ("song/chainItemEdit", &["laneFolder", "laneFileFor", "laneEventsAt"]),
];

fn mf(f: &ModuleFile) -> Json {
    json!({ "path": f.path, "rel": f.rel })
}

fn mf_opt(f: Option<ModuleFile>) -> Json {
    f.map_or(Json::Null, |f| mf(&f))
}

pub fn run(cx: &mut Ctx, module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let env = cx.env;
    Some(match (module, f) {
        ("song/yaml/edit", "setFileValue") => {
            let (Some(path), Some(v)) = (path_of(at(a, 1)), val_strict(at(a, 2))) else {
                return Some(Err(Failure::Unsupported("an argument JSON cannot carry")));
            };
            done(yaml_edit::set_file_value(fs, &s(a, 0), &path, &v), |()| undefined())
        }
        ("song/yaml/edit", "deleteFileValue") => {
            let Some(path) = path_of(at(a, 1)) else {
                return Some(Err(Failure::Unsupported("an argument JSON cannot carry")));
            };
            done(yaml_edit::delete_file_value(fs, &s(a, 0), &path), |()| undefined())
        }
        ("song/yaml/workspaceDoc", "wsText") => done(wd::ws_text(fs, &s(a, 0)), |t| json!(t)),
        ("song/yaml/workspaceDoc", "wsData") => done(wd::ws_data(fs, &s(a, 0)), |v| json_of(&v)),
        ("song/yaml/workspaceDoc", "wsWriteText") => done(wd::ws_write_text(fs, &s(a, 0), &s(a, 1)), |b| json!(b)),
        ("song/yaml/workspaceDoc", "wsWriteData") => {
            let Some(v) = val_strict(at(a, 1)) else {
                return Some(Err(Failure::Unsupported("an argument JSON cannot carry")));
            };
            let header = string_list(&at(a, 2)["header"]);
            let opts = StringifyOptions {
                header: &header,
                inline: None,
            };
            done(wd::ws_write_data(fs, &s(a, 0), &v, &opts), |b| json!(b))
        }
        ("song/yaml/workspaceFile", "wsExists") => Ok(json!(wf::ws_exists(fs, &s(a, 0)))),
        ("song/yaml/workspaceFile", "isDocName") => Ok(json!(wf::is_doc_name(&s(a, 0)))),
        ("song/yaml/workspaceFile", "wsRead") => done(wf::ws_read(fs, &s(a, 0)), |t| json!(t)),
        ("song/yaml/workspaceFile", "wsWrite") => match a.get(1) {
            Some(Json::String(t)) => done(wf::ws_write(fs, &s(a, 0), t.as_bytes()), |()| undefined()),
            Some(b) => match bytes_of(b) {
                Some(bytes) => done(wf::ws_write(fs, &s(a, 0), &bytes), |()| undefined()),
                None => Err(Failure::Unsupported("a binary argument")),
            },
            None => Err(Failure::Unsupported("a binary argument")),
        },
        ("song/yaml/workspaceFile", "wsRemove") => done(wf::ws_remove(fs, &s(a, 0)), |()| undefined()),
        ("song/yaml/workspaceFile", "wsCopy") => done(wf::ws_copy(fs, &s(a, 0), &s(a, 1)), |()| undefined()),
        ("song/yaml/workspaceFile", "k7xData") => done(wf::k7x_data(&s(a, 0), &s(a, 1)), |v| json_of(&v)),
        ("song/yaml/moduleIO", "entityFileFor" | "moduleFileFor") => {
            Ok(mf_opt(mio::module_file_for(fs, &s(a, 0), &s(a, 1), &s(a, 2))))
        }
        ("song/yaml/moduleIO", "entityFiles") => Ok(Json::Array(
            mio::entity_files(fs, &s(a, 0), &s(a, 1))
                .iter()
                .map(|(id, f)| json!({ "file": mf(f), "id": id }))
                .collect(),
        )),
        ("song/yaml/moduleIO", "nodeFiles") => Ok(Json::Array(
            mio::node_files(fs, &s(a, 0), &s(a, 1))
                .iter()
                .map(|(n, f)| json!({ "file": mf(f), "node": n }))
                .collect(),
        )),
        ("song/yaml/moduleIO", "isProjectFolder") => Ok(json!(mio::is_project_folder(fs, &s(a, 0)))),
        ("song/yaml/moduleIO", "readModuleFile") => {
            let file = ModuleFile {
                path: at(a, 0)["path"].as_str().unwrap_or("").to_owned(),
                rel: at(a, 0)["rel"].as_str().unwrap_or("").to_owned(),
            };
            done(
                mio::read_module_file(fs, &file),
                |r| json!({ "issues": r.issues, "value": r.value.as_ref().map_or(Json::Null, json_of) }),
            )
        }
        ("song/formatStamp", "stampFormatLevel") => Ok(json!(format_stamp::stamp_format_level(
            fs,
            &s(a, 0),
            at(a, 1).as_u64().unwrap_or(0) as u32
        ))),
        ("song/confine", "confineTo") => {
            let roots = string_list(at(a, 0));
            Ok(confine::confine_to(fs, &roots, &s(a, 1)).map_or(Json::Null, |p| json!(p)))
        }
        ("song/confine", "projectRoots") => Ok(json!(confine::project_roots(fs, env, &s(a, 0)))),
        ("song/confine", "workspaceDirOf") => Ok(json!(confine::workspace_dir_of(env, &s(a, 0)))),
        ("song/confine", "confineToProject") => {
            let base = at(a, 2).as_str();
            Ok(confine::confine_to_project(fs, env, &s(a, 0), &s(a, 1), base).map_or(Json::Null, |p| json!(p)))
        }
        ("song/nodeFiles", "copyNodeFiles") => done(
            node_files::copy_node_files(fs, &s(a, 0), &s(a, 1), &s(a, 2), &s(a, 3), &s(a, 4)),
            |w| json!(w),
        ),
        ("song/chainItemEdit", "laneFolder") => Ok(json!(cie::lane_folder(fs, &s(a, 0)))),
        ("song/chainItemEdit", "laneFileFor") => Ok(mf_opt(cie::lane_file_for(fs, &s(a, 0), &s(a, 1)))),
        ("song/chainItemEdit", "laneEventsAt") => done(cie::lane_events_at(fs, &s(a, 0), &s(a, 1)), |r| {
            r.map_or(
                Json::Null,
                |e| json!({ "base": e.base.iter().map(seg_json).collect::<Vec<_>>(), "path": e.path }),
            )
        }),
        _ => return None,
    })
}

/// Bytes as the stubs send them: `{"$":"bytes","b64":"..."}`.
fn bytes_of(j: &Json) -> Option<Vec<u8>> {
    if j.get("$")?.as_str()? != "bytes" {
        return None;
    }
    let text = j.get("b64")?.as_str()?;
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| *c != b'=') {
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
        }
    }
    Some(out)
}
