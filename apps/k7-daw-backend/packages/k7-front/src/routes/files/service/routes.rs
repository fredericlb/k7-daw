//! routes.rs — the front's one table of the file service's routes: a JSON route its native piece answers, a stream,
//! or a route a piece's hook answers with the request whole; a piece that declines refuses with a sentence (4-7).
//! What a reader gets wrong: a hooked path no hook answers is a 404 unless a piece owns it (`service/mod.rs`).

use std::sync::Arc;

use axum::response::Response;
use hyper::Method;
use serde_json::Value;

use super::body::{self, Field};
use super::native;

/// A JSON route the native pieces answer, its body read here first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H {
    FileGet,
    FilePut,
    Fmt,
    ModuleUi,
    Consoles,
    ModuleEngines,
    Folder,
    Rename,
    Move,
    LibraryMove,
    EntryDelete,
    ModuleClone,
    Node,
    ProjectCreate,
    Module,
    History,
    HistoryEntries,
    Undo,
    Redo,
    Project,
    ProjectDoc,
    EngineDoc,
    Graph,
    Surfaces,
    ProjectErrors,
    Revision,
    Tree,
    Hints,
    BaseModules,
    IoPorts,
    ModuleCatalogue,
    Presets,
    ModuleBase,
    ProjectPlugins,
    ProjectCompat,
    LibraryChains,
    LibraryAssemblies,
    Playlists,
    Resources,
    References,
    Changes,
    /// A foreign project imported into a new folder (`native/import_run.rs`).
    Import,
    /// A command line run natively (`native/verb_run.rs`).
    Verb,
    /// The completions of a line under the caret, by the `verbs` crate.
    VerbComplete,
    /// The palette's catalogue, read from the `verbs` crate.
    VerbCatalogue,
    /// The open project's picture document, by the `verbs` crate (`native/picture_run.rs`).
    Picture,
    /// …and the scene the page draws or exports of it.
    PictureScene,
    VerbBundle,
    VerbLibrary,
    /// A host member's route (3f).
    VerbHost(native::verb_host::Member),
}

/// A route whose body or answer is a stream of bytes, not JSON (`stream.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Upload,
    Media,
    /// A record's envelope, read off the workspace file `/media` would send (`record.rs`).
    Envelope,
}

/// Who answers a request.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Handler(H),
    Stream(Stream),
    /// A route one of the pieces' hooks answers (`service/mod.rs`), or none does and it is a 404.
    Hooked,
}

/// The table: the JSON routes, the two streams, and every other path handed to the hooks.
pub fn route(method: &Method, path: &str) -> Route {
    let get = method == Method::GET;
    let post = method == Method::POST;
    match (path, get, post) {
        ("/media", true, _) => return Route::Stream(Stream::Media),
        ("/record/envelope", true, _) => return Route::Stream(Stream::Envelope),
        ("/upload", _, true) => return Route::Stream(Stream::Upload),
        _ => {}
    }
    let h = match (path, get, post) {
        ("/file", true, _) => H::FileGet,
        ("/file", _, _) if method == Method::PUT => H::FilePut,
        ("/fmt", _, true) => H::Fmt,
        ("/module-ui", true, _) => H::ModuleUi,
        ("/consoles", true, _) => H::Consoles,
        ("/module-engines", true, _) => H::ModuleEngines,
        ("/folder", _, true) => H::Folder,
        ("/rename", _, true) => H::Rename,
        ("/move", _, true) => H::Move,
        ("/library/move", _, true) => H::LibraryMove,
        ("/entry/delete", _, true) => H::EntryDelete,
        ("/module/clone", _, true) => H::ModuleClone,
        ("/node", _, true) => H::Node,
        ("/project/create", _, true) => H::ProjectCreate,
        ("/module", _, true) => H::Module,
        ("/history", true, _) => H::History,
        ("/history/entries", true, _) => H::HistoryEntries,
        ("/undo", _, true) => H::Undo,
        ("/redo", _, true) => H::Redo,
        ("/project", true, _) => H::Project,
        ("/project-doc", true, _) => H::ProjectDoc,
        ("/engine-doc", true, _) => H::EngineDoc,
        ("/graph", true, _) => H::Graph,
        ("/surfaces", true, _) => H::Surfaces,
        ("/project-errors", true, _) => H::ProjectErrors,
        ("/revision", true, _) => H::Revision,
        ("/tree" | "/workspace-tree", true, _) => H::Tree,
        ("/hints", true, _) => H::Hints,
        ("/base-modules", true, _) => H::BaseModules,
        ("/io-ports", true, _) => H::IoPorts,
        ("/module-catalogue", true, _) => H::ModuleCatalogue,
        ("/presets", true, _) => H::Presets,
        ("/module-base", true, _) => H::ModuleBase,
        ("/project-plugins", true, _) => H::ProjectPlugins,
        ("/project-compat", true, _) => H::ProjectCompat,
        ("/library-chains", true, _) => H::LibraryChains,
        ("/library-assemblies", true, _) => H::LibraryAssemblies,
        ("/playlists", true, _) => H::Playlists,
        ("/resources", true, _) => H::Resources,
        ("/references", true, _) => H::References,
        ("/changes", _, true) => H::Changes,
        ("/import", _, true) => H::Import,
        ("/verb", _, true) => H::Verb,
        ("/verb/complete", _, true) => H::VerbComplete,
        ("/verb/catalogue", true, _) => H::VerbCatalogue,
        ("/picture", _, true) => H::Picture,
        ("/picture/scene", _, true) => H::PictureScene,
        ("/verb-bundle", true, _) => H::VerbBundle,
        ("/verb-bundle/library", true, _) => H::VerbLibrary,
        _ => match native::verb_host::Member::of(method, path) {
            Some(m) => H::VerbHost(m),
            None => return Route::Hooked,
        },
    };
    Route::Handler(h)
}

/// Whether a route writes inside the workspace its header names: a workspace with a merge waiting refuses it (`docs/files/git.md`).
pub fn writes(h: H) -> bool {
    matches!(
        h,
        H::FilePut
            | H::Folder
            | H::Rename
            | H::Move
            | H::EntryDelete
            | H::ModuleClone
            | H::Node
            | H::ProjectCreate
            | H::Undo
            | H::Redo
            | H::Changes
            | H::Import
            | H::Verb
            | H::LibraryMove
            | H::Module
    ) || matches!(h, H::VerbHost(m) if m.writes())
}

/// The most bytes of body a route reads: 8 MiB as `express.json` takes, less for a host member that needs less.
pub fn body_limit(h: H) -> usize {
    match h {
        H::VerbHost(m) => m.limit(),
        H::Import => native::import_run::BODY_LIMIT,
        _ => body::JSON_LIMIT,
    }
}

/// One request a native piece answers: its account, its workspace, what it carried.
#[derive(Clone)]
pub struct Call {
    /// The account's ring, which the Logs panel reads; none in a test with no runtime.
    pub runtime: Option<Arc<super::native::runtime::Runtime>>,
    pub workspace: String,
    pub root: String,
    pub query: String,
    pub body: Value,
    pub log: k7_hub::Log,
    /// The native read of this request; `None` only for a route no tag names, which is refused.
    pub native: Option<super::native::Ctx>,
    /// The account's media: a native move cancels its prewarm itself (2431).
    pub media: Option<Arc<super::native::media::Media>>,
    /// The account's engine link: a write inside the open project pushes through it.
    pub engine: Option<Arc<super::native::engine::routes::Engine>>,
    /// The front's own routes asked as this request's caller, for a verb's host members; none outside `POST /verb`.
    pub reenter: Option<Reenter>,
    /// The bytes of a dropped file, for `POST /import` alone: the body was no JSON.
    pub raw: Option<Arc<Vec<u8>>>,
}

/// One request to the front with the caller's headers: its method, its target (path and query) and its JSON body.
pub type Reenter = Arc<
    dyn Fn(
            hyper::Method,
            String,
            Value,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
        + Send
        + Sync,
>;

impl Call {
    /// A line node's service would say in its own process: the front's log, and the Logs panel's (`serviceLine`).
    pub(super) fn say(&self, level: &str, line: &str) {
        (self.log)(line);
        if let Some(r) = &self.runtime {
            r.say(level, line);
        }
    }
}

/// `validate(schema)`'s 400, or nothing.
pub(super) fn invalid(c: &Call, schema: &[(&str, Field)]) -> Option<Response> {
    body::check(&c.body, schema)
        .err()
        .map(|m| crate::routes::files::json_answer(400, &serde_json::json!({ "error": m })))
}

pub(super) const PATH: [(&str, Field); 1] = [("path", Field::Text)];
pub(super) const PATH_NAME: [(&str, Field); 2] = [("path", Field::Text), ("name", Field::Text)];
pub(super) const PATH_TO: [(&str, Field); 2] = [("path", Field::Text), ("toDir", Field::Text)];
/// `PROJECT_SHAPES` (`apps/k7-daw-frontend/packages/song-schema/src/projectShapes.ts`), held to it by a test.
pub(super) const SHAPES: &[&str] = &["arrangement", "mix", "video", "live", "live-video"];

/// Whether a route answers a compile of the project, and so names the compiler that made it (decision 758).
pub fn reports_compiler(h: H) -> bool {
    matches!(
        h,
        H::ProjectDoc
            | H::EngineDoc
            | H::ProjectErrors
            | H::Graph
            | H::Surfaces
            | H::IoPorts
            | H::ModuleCatalogue
            | H::BaseModules
            | H::Hints
    )
}

/// The route's answer: its piece's, or the refusal of its decline (4-7).
pub async fn dispatch(h: H, c: &Call) -> Response {
    let answered = match c.native.as_ref() {
        Some(n) => native::answer(h, c, n).await,
        None => None,
    };
    answered.unwrap_or_else(native::refuse::refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route node's own routers answered, which a piece's hook answers now (`live()` before step 5).
    const HOOKED: &[(&str, &str)] = &[
        ("GET", "/workspace"),
        ("GET", "/engine/served"),
        ("POST", "/open"),
        ("POST", "/project/delete"),
        ("POST", "/project/rename"),
        ("POST", "/engine/project"),
        ("POST", "/engine/unload"),
        ("POST", "/engine/sync/retry"),
        ("POST", "/auto-freeze/thaw"),
        ("GET", "/workspaces"),
        ("POST", "/workspaces"),
        ("POST", "/workspaces/remove"),
        ("POST", "/workspaces/copy-project"),
        ("POST", "/workspaces/bench"),
        ("GET", "/workspaces/projects"),
        ("GET", "/workspaces/surfaces"),
        ("GET", "/preferences"),
        ("GET", "/module-libraries"),
        ("PUT", "/preferences/vim"),
        ("GET", "/account/live-chains"),
        ("GET", "/account/frozen-chains"),
        ("GET", "/account/cold-chains"),
        ("POST", "/account/slot"),
        ("POST", "/bench/save"),
        ("GET", "/waveform"),
        ("GET", "/waveforms/status"),
        ("POST", "/waveforms/prewarm"),
        ("POST", "/waveforms/cancel"),
        ("POST", "/preview"),
        ("POST", "/module-base/snapshot"),
        ("GET", "/plugin-params"),
        ("GET", "/plugin-native-ui"),
        ("GET", "/logs"),
        ("GET", "/export"),
        ("GET", "/export/tracks"),
        ("POST", "/models/download"),
        ("POST", "/account/frozen-chains/x/copy"),
        ("DELETE", "/account/frozen-chains/x"),
        ("PUT", "/account/cold-chains/stems"),
        ("POST", "/account/cold-chains/download"),
        ("GET", "/account/models/abc"),
    ];

    #[test]
    fn one_table_names_the_json_routes_the_streams_and_hands_the_rest_to_the_hooks() {
        assert_eq!(route(&Method::GET, "/file"), Route::Handler(H::FileGet));
        assert_eq!(route(&Method::PUT, "/file"), Route::Handler(H::FilePut));
        assert_eq!(
            route(&Method::GET, "/workspace-tree"),
            Route::Handler(H::Tree)
        );
        assert_eq!(route(&Method::GET, "/media"), Route::Stream(Stream::Media));
        assert_eq!(
            route(&Method::POST, "/upload"),
            Route::Stream(Stream::Upload)
        );
        for (method, path) in HOOKED {
            let m = Method::from_bytes(method.as_bytes()).unwrap();
            assert_eq!(route(&m, path), Route::Hooked, "{method} {path}");
            assert!(
                native::refuse::owned(&m, path),
                "{method} {path} has no piece"
            );
        }
    }

    #[test]
    fn a_path_no_piece_owns_is_hooked_and_owned_by_none() {
        // The importers run in the browser's Worker: neither of their old routes is the file service's any more.
        for (method, path) in [
            (Method::POST, "/file"),
            (Method::GET, "/nothing-here"),
            (Method::GET, "/account/models/x/y"),
            (Method::GET, "/waveforms/prewarm"),
            (Method::GET, "/bench/save"),
            (Method::PUT, "/account/cold-chains/download"),
            (Method::POST, "/import-project"),
            (Method::POST, "/import-upload"),
            (Method::GET, "/sync/log"),
            (Method::GET, "/sync/project"),
            (Method::GET, "/sync/conflicts"),
            (Method::POST, "/sync/conflicts/x"),
            (Method::GET, "/workspaces/sync"),
            (Method::POST, "/workspaces/sync/download"),
        ] {
            assert_eq!(route(&method, path), Route::Hooked, "{method} {path}");
            assert!(!native::refuse::owned(&method, path), "{method} {path}");
        }
    }

    #[test]
    fn knows_every_archetype_the_typescript_does() {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../..")
            .join("apps/k7-daw-frontend/packages/song-schema/src")
            .join("projectShapes.ts");
        let ts = std::fs::read_to_string(at).unwrap();
        let list = &ts[ts.find("PROJECT_SHAPES = [").unwrap()..];
        let list = &list[..list.find(']').unwrap()];
        let names: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        assert_eq!(names, SHAPES);
    }
}
