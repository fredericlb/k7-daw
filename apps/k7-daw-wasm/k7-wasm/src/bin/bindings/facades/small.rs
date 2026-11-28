// facades/small.rs — the wrappers of the small families: records, track, stems, library, surfaces, nodes, fields, rules,
// module, picture, engine, eyes, media and file. A reader would get wrong that an op missing here has no caller: it is wrapped by hand.

use crate::facade::{Facade, bare, facade, op, whole};
use k7_wasm::ops::{
    engine, eyes, fields, file, library, media, module, nodes, picture, records, rules, schema, stems, surfaces, track,
};
use ts_rs::TS;

#[rustfmt::skip]
pub fn all() -> Vec<Facade> {
    vec![
        facade("records", vec![records::RecordsRequest::name()], vec![
            op::<records::LineArgs>("recordPathOf", "records.path_of", "string | null",
                "A line as the workspace path of its record, its slashes trimmed; null for one that leaves the workspace or names a library (`@`)."),
            op::<records::PathArgs>("recordName", "records.name", "string", "What a row is called: the file's name without its extension."),
            op::<records::PathArgs>("recordFolder", "records.folder", "string | null", "…and the folder holding it, or null for a record at the root."),
            op::<records::MatchArgs>("recordMatches", "records.matches", "boolean", "Does this record match what was typed? Every word must be somewhere in its path."),
            op::<records::AudioFilesArgs>("audioFilesOf", "records.audio_files_of", "string[]",
                "A tree's audio files in tree order; a `stems/` folder's are left out unless asked, being no record."),
        ]),
        facade("track", vec![track::TrackRequest::name()], vec![
            op::<track::TrackNameArgs>("trackLabel", "track.label", "string",
                "The name with its position and instrument stripped, what a view SHOWS; the whole name when nothing would remain."),
            op::<track::TrackNameArgs>("trackKind", "track.kind", "TrackKind | null",
                "The instrument a name declares — its code, family and tooltip label — or null when it declares none."),
        ]),
        facade("stems", vec![stems::StemsRequest::name()], vec![
            op::<stems::DeckStemArgs>("stemLane", "stems.lane", "string", "The lane one stem plays on: `stemBassA`."),
            op::<stems::DeckStemArgs>("stemPlatter", "stems.platter", "string", "…and the PLATTER that reads its file: `platterBassA`."),
            op::<stems::DeckStemArgs>("stemControl", "stems.control", "string", "The desk control that drives a stem's node: `deckA.stemBass`."),
        ]),
        facade("library", vec![library::LibraryRequest::name()], vec![
            op::<library::TreeArgs>("libraryRows", "library.tree", "unknown[]",
                "The libraries as file rows (the page types them `FileNode`): a level or a repository with its families."),
            op::<library::LevelsArgs>("readOnlyPathList", "library.readOnly", "string[]", "Every path under a READ-ONLY level, plus the level's own row."),
            op::<library::RootArgs>("rootIndexOf", "library.rootOf", "number | null",
                "The index of the level a path is the root of or lies under, null under a read-only level or none."),
            op::<library::PathArgs>("fileFolderOf", "library.fileFolder", "string | null",
                "The family folder (`samples`, `irs`, `wavetables`, `models`) a row is or lies in, null elsewhere."),
            op::<library::PathArgs>("libraryModuleRef", "library.moduleRef", "string | null",
                "The ref a chain writes for one of a SHARED module's files, null when the path names none."),
            op::<library::PathArgs>("isLibraryPath", "library.isPath", "boolean", "Is this workspace path inside one of the ACCOUNT's libraries?"),
            op::<library::PathArgs>("libraryOfPath", "library.ofPath", "string | null", "WHICH library a `_libraries/<name>/…` path is in, or null."),
        ]),
        facade("surfaces", vec![surfaces::SurfacesRequest::name()], vec![
            whole::<chains::surface_rows::SurfaceRowsArgs>("surfaceGroupsOf", "surfaces.rows", "RowGroup[]",
                "One group per device, in the order the peers came, each row an address (as its pieces) or the one reason it is not."),
            op::<surfaces::InUseArgs>("inUseBy", "surfaces.inUse", "InUse",
                "The device holding each control the remotes show; where two show one surface, the one that came first."),
            op::<surfaces::NextFreeArgs>("nextFree", "surfaces.nextFree", "string | null",
                "The next pane after `from` in `order`, wrapping, that is not blocked; null when every other one is."),
            op::<surfaces::SameScreensArgs>("sameScreens", "surfaces.sameScreens", "boolean",
                "Whether two lists name the same screens, so a poll that changed nothing re-renders nothing."),
            op::<surfaces::HistoryArgs>("historyMarksOf", "surfaces.history", "HistoryMarkLists",
                "What one History row touched, from its project-relative `paths` and a journal step's `label`."),
            op::<surfaces::MasterModulesArgs>("masterModules", "surfaces.masterModules", "string[]",
                "The modules a `mastering` mapping names, deduplicated, in the order met."),
        ]),
        facade("nodes", vec![nodes::NodesRequest::name()], vec![
            op::<nodes::ToggleArgs>("toggleSelection", "nodes.toggleSelection", "string[]",
                "Shift+click on `name`: the selection with it toggled, the single selection it grew from counted in."),
            op::<nodes::LinkedArgs>("linkedNodeNames", "nodes.linked", "string[]",
                "The names of the OTHER selected cards a control on `of` drives too: none unless all share one ref and one engine."),
            op::<nodes::BoxArgs>("nodesInBox", "nodes.inBox", "string[]", "The module names whose cards a rectangle TOUCHES, in world coordinates."),
            op::<nodes::ShownArgs>("shownNameMap", "nodes.shownNames", "Record<string, string>",
                "Each placed node's shown name, by node name; a node no step places is absent."),
            op::<nodes::GroupEdgeArgs>("groupEdgeInsets", "nodes.groupEdge", "Record<string, number>",
                "Each widget drawn inset, by id: framed by a group that draws a name or a switch, on its left edge."),
            op::<nodes::SplitArgs>("splitTrailingMixer", "nodes.splitMixer", "SplitMixer | null",
                "A row without its last card, and that card alone; null when the row does not END on a lane's mixer."),
        ]),
        facade("fields", vec![fields::FieldsRequest::name()], vec![
            op::<fields::AtArgs>("fieldsAt", "fields.at", "string[]",
                "The fields allowed in the mapping at `at`, the editor's key path (`[]` at the top level)."),
            op::<fields::ValuesArgs>("valuesAt", "fields.values", "string[]",
                "The values allowed for a field whose schema is a closed set; empty for free text."),
        ]),
        facade("rules", vec![rules::RulesRequest::name()], vec![
            op::<rules::SeqWindowArgs>("seqWindowFromText", "rules.seq_window", "SeqWindow | null",
                "The window `text` states in beats (in 4/4 unless told), or null when it states none: a half-typed edge reads as absent, not as 0."),
            op::<rules::WindowSpanArgs>("windowSpan", "rules.window_span", "WindowSpan | null",
                "Both edges as beats against the notes' extent; null when the span does not run forwards."),
            op::<schema::TextArgs>("projectTagsOf", "rules.project_tags", "string[]",
                "The tags of a project.k7x's text; none for a file that does not parse, since a listing must not fail on one."),
        ]),
        facade("module", vec![module::ModuleRequest::name()], vec![
            op::<records::PathArgs>("moduleKeyFromPath", "module.key_from_path", "string | null",
                "The chain KEY of the module a path points into: everything between the type folder and the filename."),
            op::<module::LibraryRefArgs>("libraryRef", "module.library_ref", "string",
                "The ref a library gives the thing at `rest` inside it: `@k7-core/audio/reverb`."),
            op::<module::ParamArgs>("shaderUniformFor", "module.shader_uniform", "string",
                "The uniform each shader knob lands on: one rule, so a panel and a shader cannot disagree."),
            op::<module::AssetsArgs>("assetsForParam", "module.assets_for_param", "string[]",
                "Those of `files` a param of this name can take; a panel naming its `family` filters by that instead."),
        ]),
        facade("picture", vec![picture::PictureRequest::name()], vec![
            op::<picture::ViewportArgs>("aspectOf", "picture.aspect_of", "Aspect | null",
                "Which shape a viewport IS, by ratio rather than by exact size; null when it matches none."),
            op::<picture::ResolveSettingsArgs>("resolveVideoSettings", "picture.resolve_settings", "ResolvedVideo",
                "The range and file size `settings` resolve to on a scene: the whole song by default, a height alone taking the viewport's aspect."),
        ]),
        facade("engine", vec![engine::EngineRequest::name()], vec![
            op::<engine::HashArgs>("textHash", "text.hash", "string",
                "A short fingerprint of a text, the same on every side of a write: what a write sends as the base it was edited from."),
            op::<engine::CompatArgs>("compatibility", "engine.compat", "Compatibility",
                "The share of a project's plugins an engine's catalogue has, and which are missing."),
            op::<engine::LatencyArgs>("engineLatency", "engine.latency", "EngineLatency",
                "A link's latency in ms and its band: the round trip plus twice the buffer period."),
            op::<engine::WarningArgs>("readWarning", "engine.warning", "WarningAnswer",
                "What an engine warning's prose is about, and the missing thing it reports, if any."),
            op::<engine::ChooseArgs>("chooseEngineAt", "engine.choose", "Choice",
                "The engine to use from what the registry offered, as an index; null index when the user has to be asked."),
        ]),
        facade("eyes", vec![eyes::EyesRequest::name()], vec![
            op::<eyes::ScheduleArgs>("layerSchedule", "eyes.schedule", "LayerSchedule",
                "The table of spans a layer's clips make, which the compositor looks up per frame instead of working the rule out."),
            bare("builtinTransitions", "eyes.builtins", "Record<string, SceneTransition>",
                "The two transitions the compositor owns, `dissolve` and `over`, keyed by name."),
            op::<eyes::LensArgs>("matchCamera", "eyes.match_camera", "CameraMatch",
                "Which lens a layer's label names among the machine's devices; a lens that is not here falls back, with the reason."),
        ]),
        facade("media", vec![media::MediaRequest::name()], vec![
            op::<media::TrimArgs>("trimWindow", "media.trimWindow", "TimeSpan | null",
                "The span of an envelope above a gate in dBFS, in seconds, caged to `within`; null when nothing sounds."),
        ]),
        facade("file", vec![file::FileRequest::name()], vec![
            op::<file::ValidateArgs>("validateFile", "file.validate", "Validity",
                "Is `text` a document the compiler could read at `path`? A newer format first, then syntax, then the schema."),
        ]),
    ]
}
