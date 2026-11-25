//! tag.rs — the routes the native pieces answer, by the tag they carry in a line of the log, and the piece each belongs to.
//! What a reader gets wrong: a tag is a route, not a piece; `piece()` is the one place a route is tied to its piece.

use crate::routes::files::service::routes::H;

/// The parts of the file service, each answering its own routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Reads,
    Docs,
    Listings,
    Entries,
    Revision,
    Journal,
    Workspaces,
    Projects,
    Media,
    Chains,
    Engine,
    EngineJobs,
    Shares,
    Runtime,
}

/// The routes `reads` answers, by the tag they carry in a line of the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    File,
    Fmt,
    Tree,
    Media,
    Playlists,
    References,
    LibraryChains,
    LibraryAssemblies,
    ModuleLibraries,
    Folder,
    Rename,
    LibraryMove,
    ModuleCreate,
    ModuleClone,
    NodeCreate,
    Presets,
    ModuleUi,
    Consoles,
    ModuleEngines,
    ModuleBase,
    ProjectPlugins,
    ProjectCompat,
    Resources,
    ProjectCreate,
    Move,
    Delete,
    Upload,
    Revision,
    ProjectDoc,
    EngineDoc,
    ProjectErrors,
    Graph,
    Hints,
    Surfaces,
    IoPorts,
    Catalogue,
    BaseModules,
    FilePut,
    Undo,
    Redo,
    History,
    HistoryEntries,
    Workspaces,
    WorkspaceMake,
    WorkspaceRemove,
    CopyProject,
    WsProjects,
    WsSurfaces,
    Open,
    ProjectRename,
    ProjectDelete,
    Project,
    Workspace,
    EngineProject,
    EngineUnload,
    EngineSyncRetry,
    EngineServed,
    Waveform,
    WaveformsStatus,
    WaveformsPrewarm,
    WaveformsCancel,
    AccountModel,
    ModelsDownload,
    Preview,
    PluginParams,
    PluginNativeUi,
    ModuleBaseSnapshot,
    AutoFreezeThaw,
    Export,
    ExportTracks,
    Changes,
    Verb,
    VerbComplete,
    VerbBundle,
    VerbLibrary,
    VerbHost(super::verb_host::Member),
    FrozenChains,
    LiveChains,
    ColdChains,
    ColdRun,
    ColdChainsDownload,
    ColdSet,
}

impl Tag {
    /// The route a handler is, when `reads` has it; `/tree` and `/workspace-tree` are one handler and one tag.
    pub fn of(h: H) -> Option<Tag> {
        match h {
            H::FileGet => Some(Tag::File),
            H::Fmt => Some(Tag::Fmt),
            H::Tree => Some(Tag::Tree),
            H::Playlists => Some(Tag::Playlists),
            H::References => Some(Tag::References),
            H::LibraryChains => Some(Tag::LibraryChains),
            H::LibraryAssemblies => Some(Tag::LibraryAssemblies),
            H::Folder => Some(Tag::Folder),
            H::Rename => Some(Tag::Rename),
            H::Move => Some(Tag::Move),
            H::LibraryMove => Some(Tag::LibraryMove),
            H::Module => Some(Tag::ModuleCreate),
            H::ModuleClone => Some(Tag::ModuleClone),
            H::Node => Some(Tag::NodeCreate),
            H::Presets => Some(Tag::Presets),
            H::ModuleUi | H::Consoles | H::ModuleEngines | H::ModuleBase => {
                super::orphans::tag_of(h)
            }
            H::ProjectPlugins | H::ProjectCompat | H::Resources => {
                super::orphans_project::tag_of(h)
            }
            H::ProjectCreate => Some(Tag::ProjectCreate),
            H::EntryDelete => Some(Tag::Delete),
            H::Revision => Some(Tag::Revision),
            H::ProjectDoc => Some(Tag::ProjectDoc),
            H::EngineDoc => Some(Tag::EngineDoc),
            H::ProjectErrors => Some(Tag::ProjectErrors),
            H::Graph => Some(Tag::Graph),
            H::Hints => Some(Tag::Hints),
            H::Surfaces => Some(Tag::Surfaces),
            H::IoPorts => Some(Tag::IoPorts),
            H::ModuleCatalogue => Some(Tag::Catalogue),
            H::BaseModules => Some(Tag::BaseModules),
            H::FilePut => Some(Tag::FilePut),
            H::Undo => Some(Tag::Undo),
            H::Redo => Some(Tag::Redo),
            H::History => Some(Tag::History),
            H::HistoryEntries => Some(Tag::HistoryEntries),
            H::Changes => Some(Tag::Changes),
            H::Verb => Some(Tag::Verb),
            H::VerbComplete => Some(Tag::VerbComplete),
            H::VerbBundle => Some(Tag::VerbBundle),
            H::VerbLibrary => Some(Tag::VerbLibrary),
            H::VerbHost(m) => Some(Tag::VerbHost(m)),
            H::Project => Some(Tag::Project),
        }
    }

    pub fn piece(self) -> Piece {
        match self {
            Tag::File | Tag::Fmt | Tag::Tree | Tag::Media => Piece::Reads,
            Tag::Folder
            | Tag::Rename
            | Tag::Move
            | Tag::LibraryMove
            | Tag::ModuleCreate
            | Tag::ModuleClone
            | Tag::NodeCreate
            | Tag::Presets
            | Tag::ModuleUi
            | Tag::Consoles
            | Tag::ModuleEngines
            | Tag::ModuleBase
            | Tag::ProjectPlugins
            | Tag::ProjectCompat
            | Tag::Resources
            | Tag::ProjectCreate
            | Tag::Delete
            | Tag::Upload => Piece::Entries,
            Tag::Revision => Piece::Revision,
            Tag::ProjectDoc
            | Tag::EngineDoc
            | Tag::ProjectErrors
            | Tag::Graph
            | Tag::Hints
            | Tag::Surfaces
            | Tag::IoPorts
            | Tag::Catalogue
            | Tag::BaseModules => Piece::Docs,
            Tag::FilePut
            | Tag::Undo
            | Tag::Redo
            | Tag::History
            | Tag::HistoryEntries
            | Tag::Changes
            | Tag::Verb
            | Tag::VerbComplete
            | Tag::VerbBundle
            | Tag::VerbLibrary
            | Tag::VerbHost(_) => Piece::Journal,
            Tag::Workspaces
            | Tag::WorkspaceMake
            | Tag::WorkspaceRemove
            | Tag::CopyProject
            | Tag::WsProjects
            | Tag::WsSurfaces => Piece::Workspaces,
            Tag::Open | Tag::ProjectRename | Tag::ProjectDelete | Tag::Project => Piece::Projects,
            Tag::Workspace
            | Tag::EngineProject
            | Tag::EngineUnload
            | Tag::EngineSyncRetry
            | Tag::EngineServed => Piece::Engine,
            Tag::Waveform
            | Tag::WaveformsStatus
            | Tag::WaveformsPrewarm
            | Tag::WaveformsCancel
            | Tag::AccountModel
            | Tag::ModelsDownload => Piece::Media,
            Tag::Preview
            | Tag::PluginParams
            | Tag::PluginNativeUi
            | Tag::ModuleBaseSnapshot
            | Tag::AutoFreezeThaw => Piece::EngineJobs,
            Tag::FrozenChains
            | Tag::LiveChains
            | Tag::ColdChains
            | Tag::ColdChainsDownload
            | Tag::ColdSet
            | Tag::ColdRun => Piece::Chains,
            Tag::Export | Tag::ExportTracks => Piece::Shares,
            _ => Piece::Listings,
        }
    }

    /// The route as a line of the log names it.
    pub fn route(self) -> &'static str {
        match self {
            Tag::File => "GET /file",
            Tag::Fmt => "POST /fmt",
            Tag::Tree => "GET /tree",
            Tag::Media => "GET /media",
            Tag::Playlists => "GET /playlists",
            Tag::References => "GET /references",
            Tag::LibraryChains => "GET /library-chains",
            Tag::LibraryAssemblies => "GET /library-assemblies",
            Tag::ModuleLibraries => "GET /module-libraries",
            Tag::Folder => "POST /folder",
            Tag::Rename => "POST /rename",
            Tag::Move => "POST /move",
            Tag::LibraryMove => "POST /library/move",
            Tag::ModuleCreate => "POST /module",
            Tag::ModuleClone => "POST /module/clone",
            Tag::NodeCreate => "POST /node",
            Tag::Presets => "GET /presets",
            Tag::ModuleUi => "GET /module-ui",
            Tag::Consoles => "GET /consoles",
            Tag::ModuleEngines => "GET /module-engines",
            Tag::ModuleBase => "GET /module-base",
            Tag::ProjectPlugins => "GET /project-plugins",
            Tag::ProjectCompat => "GET /project-compat",
            Tag::Resources => "GET /resources",
            Tag::ProjectCreate => "POST /project/create",
            Tag::Delete => "POST /entry/delete",
            Tag::Upload => "POST /upload",
            Tag::Revision => "GET /revision",
            Tag::ProjectDoc => "GET /project-doc",
            Tag::EngineDoc => "GET /engine-doc",
            Tag::ProjectErrors => "GET /project-errors",
            Tag::Graph => "GET /graph",
            Tag::Hints => "GET /hints",
            Tag::Surfaces => "GET /surfaces",
            Tag::IoPorts => "GET /io-ports",
            Tag::Catalogue => "GET /module-catalogue",
            Tag::BaseModules => "GET /base-modules",
            Tag::FilePut => "PUT /file",
            Tag::Undo => "POST /undo",
            Tag::Redo => "POST /redo",
            Tag::History => "GET /history",
            Tag::HistoryEntries => "GET /history/entries",
            Tag::Workspaces => "GET /workspaces",
            Tag::WorkspaceMake => "POST /workspaces",
            Tag::WorkspaceRemove => "POST /workspaces/remove",
            Tag::CopyProject => "POST /workspaces/copy-project",
            Tag::WsProjects => "GET /workspaces/projects",
            Tag::WsSurfaces => "GET /workspaces/surfaces",
            Tag::Open => "POST /open",
            Tag::ProjectRename => "POST /project/rename",
            Tag::ProjectDelete => "POST /project/delete",
            Tag::Project => "GET /project",
            Tag::Workspace => "GET /workspace",
            Tag::EngineProject => "POST /engine/project",
            Tag::EngineUnload => "POST /engine/unload",
            Tag::EngineSyncRetry => "POST /engine/sync/retry",
            Tag::EngineServed => "GET /engine/served",
            Tag::Waveform => "GET /waveform",
            Tag::WaveformsStatus => "GET /waveforms/status",
            Tag::WaveformsPrewarm => "POST /waveforms/prewarm",
            Tag::WaveformsCancel => "POST /waveforms/cancel",
            Tag::AccountModel => "GET /account/models/:id",
            Tag::ModelsDownload => "POST /models/download",
            Tag::Preview => "POST /preview",
            Tag::PluginParams => "GET /plugin-params",
            Tag::PluginNativeUi => "GET /plugin-native-ui",
            Tag::ModuleBaseSnapshot => "POST /module-base/snapshot",
            Tag::AutoFreezeThaw => "POST /auto-freeze/thaw",
            Tag::Export => "GET /export",
            Tag::ExportTracks => "GET /export/tracks",
            Tag::Changes => "POST /changes",
            Tag::Verb => "POST /verb",
            Tag::VerbComplete => "POST /verb/complete",
            Tag::VerbBundle => "GET /verb-bundle",
            Tag::VerbLibrary => "GET /verb-bundle/library",
            Tag::VerbHost(m) => m.route(),
            Tag::FrozenChains => "GET /account/frozen-chains",
            Tag::LiveChains => "GET /account/live-chains",
            Tag::ColdChains => "GET /account/cold-chains",
            Tag::ColdChainsDownload => "POST /account/cold-chains/download",
            Tag::ColdSet => "PUT /account/cold-chains/:which",
            Tag::ColdRun => "POST /api/internal/files/cold-run",
        }
    }
}
