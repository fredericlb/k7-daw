// ops/paths.rs — the paths.* operations: which kind of project file a path is, and a path made relative, resolved or split.
// A reader would get wrong that nothing here touches a disk: a path is judged by its text and the list of files the page already holds.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_edit::project_paths as p;
use std::collections::HashMap;

/// A rule over a path: the kind of file it names.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PathRule {
    /// A MIDI sequence file.
    Seq,
    /// A clip file, audio or video.
    Clip,
    /// A lane's file: a node, or a chain project's track.
    Track,
    /// A module file whose rack's `source:` names a file beside it.
    RackModule,
    /// A module file the builder opens.
    Module,
    /// A module folder's own `module.k7x`, the path as given.
    OwnModule,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsIsArgs {
    pub rule: PathRule,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PathsVideoTrackArgs {
    pub path: String,
    pub picture_lanes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsPathArgs {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsTextArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsProjectArgs {
    pub path: String,
    pub project: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsEntityArgs {
    pub files: Vec<String>,
    pub folder: String,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsDirArgs {
    pub project: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsHeadArgs {
    pub rest: String,
    pub n: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsGutterArgs {
    pub source: String,
    pub file: String,
    pub project: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PathsBehindArgs {
    pub content: String,
    /// The hints' `kind:name` to project-relative path map.
    pub paths: HashMap<String, String>,
    pub project_root: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub own: Option<String>,
}

/// A node of the file tree, as the page holds it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileTreeNode {
    pub path: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub dir: Option<bool>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub children: Option<Vec<FileTreeNode>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathsTreeArgs {
    pub nodes: Vec<FileTreeNode>,
}

/// One entry of the tree, folders and binaries kept.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TreeEntry {
    pub path: String,
    pub dir: bool,
}

/// The paths family's requests: `{"op": "paths.is", "args": {"rule": "seq", "path": "sequences/a.k7x"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum PathsRequest {
    #[serde(rename = "paths.is")]
    Is(PathsIsArgs),
    #[serde(rename = "paths.is_video_track")]
    IsVideoTrack(PathsVideoTrackArgs),
    #[serde(rename = "paths.rack_key")]
    RackKey(PathsPathArgs),
    #[serde(rename = "paths.entity_file")]
    EntityFile(PathsEntityArgs),
    #[serde(rename = "paths.stem")]
    Stem(PathsPathArgs),
    #[serde(rename = "paths.project_dir")]
    ProjectDir(PathsDirArgs),
    #[serde(rename = "paths.relative")]
    Relative(PathsProjectArgs),
    #[serde(rename = "paths.in_project")]
    InProject(PathsProjectArgs),
    #[serde(rename = "paths.module_file_of")]
    ModuleFileOf(PathsPathArgs),
    #[serde(rename = "paths.parent")]
    Parent(PathsPathArgs),
    #[serde(rename = "paths.base")]
    Base(PathsPathArgs),
    #[serde(rename = "paths.head_tokens")]
    HeadTokens(PathsHeadArgs),
    #[serde(rename = "paths.gutter")]
    Gutter(PathsGutterArgs),
    #[serde(rename = "paths.module_behind")]
    ModuleBehind(PathsBehindArgs),
    #[serde(rename = "paths.audio_clip")]
    AudioClip(PathsTextArgs),
    #[serde(rename = "paths.use_rack")]
    UseRack(PathsTextArgs),
    #[serde(rename = "paths.text_files")]
    TextFiles(PathsTreeArgs),
    #[serde(rename = "paths.entries")]
    Entries(PathsTreeArgs),
}

fn text_files(nodes: &[FileTreeNode], out: &mut Vec<String>) {
    for n in nodes {
        if n.dir == Some(true) {
            if let Some(c) = &n.children {
                text_files(c, out);
            }
        } else if n.kind.as_deref() != Some("binary") {
            out.push(n.path.clone());
        }
    }
}

fn entries(nodes: &[FileTreeNode], out: &mut Vec<TreeEntry>) {
    for n in nodes {
        out.push(TreeEntry {
            path: n.path.clone(),
            dir: n.dir == Some(true),
        });
        if let Some(c) = &n.children {
            entries(c, out);
        }
    }
}

pub fn dispatch(request: PathsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use PathsRequest as R;
    match request {
        R::Is(a) => write(
            out,
            &match a.rule {
                PathRule::Seq => p::is_seq_file(&a.path),
                PathRule::Clip => p::is_clip_file(&a.path),
                PathRule::Track => p::is_track_file(&a.path),
                PathRule::RackModule => p::is_rack_module_file(&a.path),
                PathRule::Module => p::is_module_file(&a.path),
                PathRule::OwnModule => p::is_own_module_file(&a.path),
            },
        ),
        R::IsVideoTrack(a) => write(out, &p::is_video_track_file(&a.path, &a.picture_lanes)),
        R::RackKey(a) => write(out, &p::rack_key_of(&a.path)),
        R::EntityFile(a) => write(out, &p::entity_file(&a.files, &a.folder, &a.id)),
        R::Stem(a) => write(out, &p::file_stem(&a.path)),
        R::ProjectDir(a) => write(out, &p::project_dir(a.project.as_deref())),
        R::Relative(a) => write(out, &p::to_project_relative(&a.path, a.project.as_deref())),
        R::InProject(a) => write(out, &p::in_active_project(&a.path, a.project.as_deref())),
        R::ModuleFileOf(a) => write(out, &p::module_file_of(&a.path)),
        R::Parent(a) => write(out, &p::parent_dir(&a.path)),
        R::Base(a) => write(out, &p::base_name(&a.path)),
        R::HeadTokens(a) => write(out, &p::head_tokens(&a.rest, a.n)),
        R::Gutter(a) => write(out, &p::resolve_gutter_source(&a.source, &a.file, a.project.as_deref())),
        R::ModuleBehind(a) => write(out, &p::module_behind(&a.content, &a.paths, &a.project_root, a.own.as_deref())),
        R::AudioClip(a) => write(out, &p::is_audio_clip_file(&a.text)),
        R::UseRack(a) => write(out, &p::find_use_rack(&a.text)),
        R::TextFiles(a) => {
            let mut v = Vec::new();
            text_files(&a.nodes, &mut v);
            write(out, &v)
        }
        R::Entries(a) => {
            let mut v = Vec::new();
            entries(&a.nodes, &mut v);
            write(out, &v)
        }
    }
}
