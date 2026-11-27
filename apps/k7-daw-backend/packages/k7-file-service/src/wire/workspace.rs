// wire/workspace.rs — what the file service answers about an open workspace: its tree, its files, the journal and the engine's load.
// A reader would get wrong that these structs are not what the routes serialise: the routes write the JSON text, and `tests/all/wire.rs` holds it to these.

use serde::{Deserialize, Serialize};

/// Which app opens a project folder; k7 owns the one suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ProjectApp {
    K7,
}

/// What a file is: `binary` is shown, not edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Text,
    Binary,
}

/// A node in the project file tree: dirs carry `children`, files a `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct FileNode {
    /// The basename.
    pub name: String,
    /// Workspace-relative, POSIX separators.
    pub path: String,
    pub dir: bool,
    /// Files only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<NodeKind>,
    /// Dirs only: the folder is a project someone can open.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub project: Option<bool>,
    /// Dirs only: which app opens it, from the `.k7` suffix.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub app: Option<ProjectApp>,
    /// Dirs only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub children: Option<Vec<FileNode>>,
}

/// A save in the read-only `_modules` library lands on a project-local copy, and the chains follow it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ForkedModule {
    /// The shared module that was edited.
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    /// The local module the copy became, what the chains now name.
    pub module: String,
    /// Workspace-relative path of the file the edit landed in.
    pub path: String,
    /// The chains repointed at the copy.
    pub chains: Vec<String>,
}

/// A module's `ui.k7x`, or a text file beside a module: `path` is where it WOULD go when it does not exist yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ModuleUiFile {
    /// The module the file belongs to.
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    /// Workspace-relative.
    pub path: String,
    pub content: String,
    pub exists: bool,
}

/// What a state of the engine's plugin scan is, while a project naming an external plugin waits for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct EngineScanState {
    pub done: u32,
    /// 0 while the folders are listed.
    pub total: u32,
    /// The plugin file being loaded, empty between loads.
    pub current: String,
    /// The scan stopped answering or ran past its bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stalled: Option<bool>,
}

/// Where the opened project stands on its way to playable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum EngineLoadPhase {
    Scanning,
    Copying,
    Applying,
    Warming,
}

/// A copy to the engine's machine: only the files it had to copy count.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineSyncState {
    pub files: u32,
    pub files_done: u32,
    pub bytes: f64,
    pub bytes_done: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub failed: Option<u32>,
    /// One line per file that did not arrive: its name, then why.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub failures: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<String>,
}

/// Whether a process is still going.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
    Running,
    Done,
    Failed,
}

/// One process the file service runs: listed while it runs, and a few seconds after it ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceProcess {
    pub id: String,
    /// What it is, as a person reads it: the command line, or the background work's tag.
    pub label: String,
    pub state: ProcessState,
    /// Milliseconds since the epoch.
    pub started_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ended_at: Option<f64>,
    /// 0..1 when the work knows how far it is; absent, it is a spinner.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub progress: Option<f64>,
    /// A word on where it stands, or why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub detail: Option<String>,
}

/// The clip the recorder wrote most recently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LastTake {
    pub clip: String,
    pub track: String,
}

/// The opened folder: one project or a workspace of several; `activeProject` is the one the DAW acts on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceInfo {
    /// The absolute path opened.
    pub root: String,
    pub root_is_project: bool,
    /// Workspace-relative (`.` for the root), or null.
    pub active_project: Option<String>,
    /// Workspace-relative project dirs.
    pub projects: Vec<String>,
    /// Only the file service learns the id, and it stays set until the next take.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub last_take: Option<LastTake>,
    /// The project's files being copied to the engine's machine before it plays.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub engine_sync: Option<EngineSyncState>,
    /// Where the opened project stands on its way to playable; null once it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub engine_load: Option<EngineLoadPhase>,
    /// The engine's plugin scan while a project naming an external plugin waits for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub engine_scan: Option<EngineScanState>,
    /// What the account's file service is busy with.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub processes: Option<Vec<ServiceProcess>>,
    /// The open project is a bundled demo, which the next start of the service overwrites.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub active_is_demo: Option<bool>,
    /// The change token as of this answer, set by `/open` alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rev: Option<String>,
}

/// A file's contents; a binary file returns an empty `content`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct FileContent {
    pub path: String,
    pub binary: bool,
    pub content: String,
}

/// What a journal step touches: a text edit, or a bounced file a confirmation guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    Text,
    Process,
}

/// What one undo/redo step touched, enough to toast and to navigate the editor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct HistoryPeek {
    pub kind: StepKind,
    pub label: String,
    /// Workspace-relative paths the step reverts.
    pub files: Vec<String>,
}

/// The shortcuts' and the toast's view of the journal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_undo: Option<HistoryPeek>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub next_redo: Option<HistoryPeek>,
}

/// One journal step as the History panel lists it; `undone` is past the cursor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct HistoryListEntry {
    pub id: u32,
    /// Milliseconds since the epoch.
    pub ts: f64,
    pub kind: StepKind,
    pub label: String,
    /// Project-relative.
    pub paths: Vec<String>,
    pub undone: bool,
}

/// The `POST /undo | /redo` reply: applied, refused pending confirmation, or nothing to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub applied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub needs_confirm: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<StepKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub files: Option<Vec<String>>,
}

/// The project the backend is serving, for the header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ProjectInfo {
    pub name: String,
    pub root: String,
}

/// A model being fetched into this machine's cache.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ModelDownloadJob {
    pub id: String,
    pub file: String,
    pub url: String,
    pub received: f64,
    /// Null when the server did not announce a size.
    pub total: Option<f64>,
    pub done: bool,
    pub error: Option<String>,
}
