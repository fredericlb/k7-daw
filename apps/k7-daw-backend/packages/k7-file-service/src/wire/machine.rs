// wire/machine.rs — what a machine's file service lists: workspaces and their projects, surfaces, the served project, stems, playlists, clips.
// A reader would get wrong that these structs are not what the routes serialise: the routes write the JSON text, and `tests/all/wire.rs` holds it to these.

use super::workspace::{FileNode, ProjectApp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A workspace under the studio's root, as the picker lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSummary {
    /// The folder name, which is the workspace name the user picks.
    pub name: String,
    /// Project folders inside, at any depth.
    pub project_count: u32,
    /// The folder itself is a project.
    pub is_single_project: bool,
}

/// The reply to `GET /workspaces`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct WorkspaceList {
    pub workspaces: Vec<WorkspaceSummary>,
}

/// One project inside a repository; the folder's NAME decides which app opens it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoProject {
    pub path: String,
    pub app: ProjectApp,
    /// `project.tags` of the project's `project.k7x`; absent when it has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tags: Option<Vec<String>>,
    /// The TYPES of the surfaces it declares (`timeline`, `turntables`…).
    pub surfaces: Vec<String>,
    /// Whether it is its workspace's active project, the one the engine plays.
    pub active: bool,
    /// When a file in it last changed on this replica, epoch ms, from a walk two levels deep.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub edited_at: Option<f64>,
    /// A bundled demo, overwritten on the service's next start.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub demo: Option<bool>,
}

/// One workspace on the machine with the projects it holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountWorkspace {
    pub name: String,
    pub is_single_project: bool,
    pub projects: Vec<RepoProject>,
}

/// The reply to `GET /workspaces/projects`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AccountWorkspaceList {
    pub workspaces: Vec<AccountWorkspace>,
}

/// One surface a project declares, named and typed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SurfaceRef {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub type_: String,
}

/// One project declaring at least one surface, identity fields flattened onto it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SurfacedProject {
    pub workspace: String,
    pub path: String,
    pub app: ProjectApp,
    pub active: bool,
    pub surfaces: Vec<SurfaceRef>,
}

/// The reply to `GET /workspaces/surfaces`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SurfacedProjectList {
    pub projects: Vec<SurfacedProject>,
}

/// What the session actually LOADED, which is not `SurfacedProject.active`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ServedProject {
    pub workspace: String,
    /// Workspace-relative, `.` for a lone-project workspace.
    pub path: String,
}

/// The reply to `GET /engine/served`; null means the engine holds nothing for this person.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ServedProjectReply {
    pub project: Option<ServedProject>,
}

/// How a stem separation is doing, and the `files` it produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SeparateJob {
    pub id: String,
    /// The mix being split, absolute on the engine's machine.
    pub source: String,
    pub done: bool,
    pub failed: bool,
    pub error: Option<String>,
    /// The engine's word for where it is: `sending`, `queued`, `decoding`, `separating`, `done`, `failed`…
    pub phase: String,
    /// Null while doing something with no measurable end.
    pub percent: Option<f64>,
    /// Notes worth reading: which execution provider won, and why a run failed.
    pub lines: Vec<String>,
    /// The stems written, absolute; empty until `done`.
    pub files: Vec<String>,
}

/// What `POST /stems` answers: `ready` is the disk, `job` only when there was work.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct StemsReply {
    pub ready: bool,
    pub job: Option<SeparateJob>,
}

/// The version a workspace pins of a library, and what could move it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LibraryPin {
    /// A version number, or `draft`.
    #[cfg_attr(feature = "ts", ts(type = "number | \"draft\""))]
    pub pinned: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub requires: Option<u32>,
    /// The newest this k7 could move to.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub upgrade: Option<u32>,
    /// The version played when this machine lacks the pinned one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub used: Option<u32>,
}

/// One module LIBRARY a project can reach, nearest first; `exists: false` is a state, not an absence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleLibrary {
    /// The `@`-prefixed level a chain writes: `workspace`, `k7`, `engine`.
    pub ns: String,
    /// Workspace-relative; empty for `@engine`, which has no folder.
    pub path: String,
    pub exists: bool,
    /// Nothing here can be created, renamed or written: `@engine` alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub read_only: Option<bool>,
    pub nodes: Vec<FileNode>,
    /// A library's shared SOUNDS, beside its modules.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub samples: Option<Vec<FileNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub irs: Option<Vec<FileNode>>,
    /// Single-cycle frames a panel's picker offers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub wavetables: Option<Vec<FileNode>>,
    /// The shared amp MODELS, what an amp's `model` may pick besides its own assets.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub models: Option<Vec<FileNode>>,
    /// For one of the origin's libraries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub version: Option<LibraryPin>,
}

/// One row of `GET /playlists`; `title` and `count` are parsed service-side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct PlaylistInfo {
    /// Workspace-relative, so it is already what `read` and `write` take.
    pub path: String,
    /// The file's stem, what the row falls back to and what a rename would change.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub title: Option<String>,
    pub count: u32,
    pub ok: bool,
}

/// What a clip carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Audio,
    Video,
}

/// One external resource of a project: a clip the user imported, which a lane places by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ProjectResource {
    /// The clip's id, its filename.
    pub id: String,
    pub kind: ResourceKind,
    /// The media file, workspace-relative.
    pub source: String,
    /// For a video, the lighter copy a browser plays when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub preview: Option<String>,
    /// The media file's size in bytes; 0 when it cannot be read.
    pub bytes: f64,
    /// The file is not there: the clip names something gone.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub missing: Option<bool>,
}
