// facades/editor.rs — the wrappers of the editor's families: schema, buffer and text, and the verbs' edit, ui and menu
// (the last three only in a build with `verbs`). A reader would get wrong that an op missing here has no caller: it is wrapped by hand.

use crate::facade::{Facade, facade, op};
use k7_wasm::ops::{buffer, schema, text, text_buffer, text_desk, text_lane};
use ts_rs::TS;

#[rustfmt::skip]
pub fn all() -> Vec<Facade> {
    let all = vec![
        facade("schema", vec![schema::SchemaRequest::name()], vec![
            op::<schema::RefArgs>("isEngineRef", "schema.is_engine_ref", "boolean", "Is this ref the engine's: `@engine` or anything under it, never `@engineering`."),
            op::<schema::NameArgs>("isLibraryName", "schema.is_library_name", "boolean", "Can a library be called this?"),
            op::<schema::LibraryFileArgs>("isLibraryFileOf", "schema.is_library_file_of", "boolean", "Is `path` a file a library's `family` folder holds?"),
            op::<schema::NameArgs>("isPresetName", "schema.is_preset_name", "boolean",
                "Is this a name a preset can have: its file's name, held to what every filesystem agrees on."),
            op::<schema::NameArgs>("isDocFile", "schema.is_doc_file", "boolean", "A name ending in the document extension `.k7x`, any case."),
            op::<schema::NameArgs>("stripDocExt", "schema.strip_doc_ext", "string", "`name` without its document extension; any other name unchanged."),
            op::<schema::ValueAtArgs>("valueAt", "schema.value_at", "number",
                "The value of keys sorted by time at `t`: held before the first and after the last, eased between."),
            op::<schema::IdArgs>("materialRefOf", "schema.material_ref_of", "string",
                "The material a placement id names: the last `@` before a number, a repeat's `#n` aside."),
            op::<schema::IdArgs>("timelineLaneOf", "schema.timeline_lane_of", "TimelineLane | null",
                "The timeline and lane an address `<timeline>><lane>` names, or null for a project lane."),
            op::<schema::TextArgs>("parsePlaylist", "schema.parse_playlist", "Playlist | null",
                "A workspace playlist's k7x text, or null when it does not read: an unreadable file is not an empty one."),
            op::<schema::NameArgs>("playlistPathOf", "schema.playlist_path_of", "string | null",
                "A playlist name as the workspace path holding it, null for one that would leave the folder."),
            op::<schema::TextArgs>("parseTasks", "schema.parse_tasks", "Task[]", "Every task line of a `TODOS.md`, in order."),
            op::<schema::SetCompSpanArgs>("setCompSpan", "schema.set_comp_span", "CompSpan[]",
                "The comp with `span` laid over it: what it covers is taken from every other span, the base's own dropped."),
        ]),
        facade("buffer", vec![buffer::BufferRequest::name()], vec![
            op::<buffer::BufferReadsArgs>("readBuffer", "buffer.reads", "BufferReads",
                "The three reads (a slice rack, an input's io, its channels): one parse, and none when the text cannot hold the keys they look for."),
            op::<buffer::BufferTextArgs>("k7xCursor", "buffer.cursor", "K7xCursor",
                "Where the cursor sits in key terms, read from the text BEFORE it by a tolerant scan: completion runs on half-written text."),
            op::<buffer::BufferScalarArgs>("readScalarField", "buffer.scalar", "string | null",
                "A string field's value, a call's `key(\"v\")` or an object's `key: \"v\"`, found by a scan so a half-typed file still answers."),
            op::<buffer::BufferTextArgs>("valueTokensOf", "buffer.value_tokens", "ValueToken[]",
                "Every non-empty string value of the tree with its field; empty for a text that does not read."),
            op::<buffer::BufferTextArgs>("keyTokensOf", "buffer.key_tokens", "KeyToken[]",
                "Every non-empty map key of the tree with the keys above it; empty for a text that does not read."),
            op::<buffer::BufferTextArgs>("previewTriggersOf", "buffer.preview_triggers", "PreviewTrigger[]",
                "The lines that earn a ▶, spans in the tree's coordinates, a slice after the source it belongs to."),
            op::<buffer::BufferTextArgs>("scanSlices", "buffer.slices", "SliceSym[] | null",
                "Each slice symbol's window and where its numbers and name sit; null for a file that is no slice rack or does not parse."),
        ]),
        facade("text", vec![text::TextRequest::name()], vec![
            op::<text_lane::VideoListArgs>("listVideoPlacements", "text.video_list", "VideoPlacement[]",
                "The placements a picture lane declares, in file order, the ref-less ones included; an unreadable text has none."),
            op::<text::TextArgs>("mixCues", "text.mix_cues", "MixCues", "The cue points `mix.k7x` holds by song ref; none, with the issues, when the sheet does not read."),
            op::<text::TextArgs>("readMixView", "text.mix_view", "MixViewRead",
                "The desk's bindings from `views/mix.k7x`; none, with the issues, when the file does not read."),
            op::<text_desk::NameArgs>("playlistStub", "text.playlist_stub", "string", "What a playlist file that does not exist yet says: a stub that parses."),
            op::<text::TextArgs>("parseLibraryPins", "text.pins_parse", "LibraryPins", "A workspace's pins from `_library-pins.k7x`; a text that does not read is no pins."),
            op::<text_desk::PinsArgs>("stringifyLibraryPins", "text.pins_write", "string", "`_library-pins.k7x`'s text, libraries in name order so a diff stays one line."),
            op::<text::TextArgs>("readAssemblyTemplate", "text.assembly_read", "AssemblyRead",
                "What a library `assembly.k7x` promotes; none, with the issues, when the file does not read."),
            op::<text::TextArgs>("readSequenceNotes", "text.notes_read", "SequenceRead | null",
                "A sequence's notes, expanded, and the rack it names; null while the text does not read or has no notes list."),
            op::<text_buffer::NotesWriteArgs>("writeSequenceNotes", "text.notes_write", "string | null",
                "The whole notes list written back, each note's pad effects carried from the note it was; null when the text does not read."),
        ]),
    ];
    #[cfg(feature = "verbs")]
    let all = all.into_iter().chain(verbs()).collect();
    all
}

#[cfg(feature = "verbs")]
#[rustfmt::skip]
fn verbs() -> Vec<Facade> {
    use k7_wasm::ops::verb_edit as e;
    use k7_wasm::ops::verb_menu as m;
    use k7_wasm::ops::verb_ui as u;
    vec![
        facade("edit", vec![e::EditRequest::name()], vec![
            op::<e::RestArgs>("parseExportAsk", "edit.parse_export_ask", "ExportAsk | null",
                "Read the rest of a `project export` line, or null when it says nothing this understands."),
            op::<e::PendingOfArgs>("pendingOf", "edit.pending_of", "PendingEdit[]", "The edits `line` makes: none for any other verb, or for a line that does not parse."),
            op::<e::MovedArgs>("reloadMoved", "edit.reload_moved", "boolean", "Whether a reload that wrote `files` (null: unknown) concerns `scope`."),
            op::<e::LaneArgs>("laneOfArrow", "edit.lane_of_arrow", "string | null", "The lane a `+` on this arrow drops onto, or null on the bare canvas."),
            op::<e::NodeNameArgs>("nodeNameFor", "edit.node_name_for", "string", "What a node of `module` dropped on this arrow should be called."),
            op::<e::ClassLineArgs>("chainRefFromCreate", "edit.chain_ref_from_create", "string | null",
                "What a chain should name, given the creating command that just ran: a node is named by its file alone."),
            op::<e::CreateArgs>("createForGraphCommand", "edit.create_for_graph_command", "string",
                "The line the `+` pre-writes: `node new`, ending where the node's name goes."),
            op::<e::WireArgs>("wireCommand", "edit.wire_command", "string | null",
                "Where the node that line makes should go: a splice on an arrow, and on the bare canvas only the pin, and only with a point."),
            op::<e::FilterOpsArgs>("filterOps", "edit.filter_ops", "string[]", "The chains filtered on the whole ref: an empty search is every chain rather than none."),
            op::<e::ProcessArgs>("processLine", "edit.process_line", "string", "The command line that runs a library chain over one clip."),
            op::<e::FilesArgs>("playlistRefs", "edit.playlist_refs", "string[]",
                "The records a set list's line may name: every audio file but a `stems/` folder's, sorted."),
            op::<e::PlaylistFieldsArgs>("playlistFieldsAt", "edit.playlist_fields_at", "string[]", "The fields a set list's file may carry at `path`."),
            op::<e::PlaylistValuesArgs>("playlistValuesAt", "edit.playlist_values_at", "string[]", "The values a set list's field may take, over the records."),
        ]),
        facade("ui", vec![u::UiRequest::name()], vec![
            op::<u::CompletionsArgs>("optionsFromCompletions", "ui.options_from_completions", "ArgOption[]",
                "`/complete`'s whole lines as one field's options: the prefix sliced back off, deduped, labels kept."),
            op::<u::LineValuesArgs>("lineArgs", "ui.line_args", "string[]", "The line's arguments, trailing blanks dropped; a blank in the middle is kept."),
        ]),
        facade("menu", vec![m::MenuRequest::name()], vec![
            op::<m::ContextMenuArgs>("contextMenuItems", "menu.context_menu", "Item[]",
                "The right-click's rows for a context, derived from the catalogue; empty means \"do not open a menu here\"."),
            op::<m::CtxArgs>("withAmbient", "menu.with_ambient", "MenuCtx", "A module the engine provides has no file anywhere: its context says `ambient`."),
            op::<m::CtxArgs>("menuTargetOf", "menu.target", "MenuTarget | null",
                "What a menu is on: the entity, and how its name is made; null for a context that names nothing."),
            op::<m::NodeMenuArgs>("nodeMenuItems", "menu.node_menu", "Item[]", "A lane's gutter menu: its module's other attribute sets, and its freeze."),
            op::<m::BuildArgs>("buildCommands", "menu.build_commands", "PaletteRow[]",
                "The palette's full catalogue: the backend's commands plus the shell's own, each sentence resolved by the locale."),
            op::<m::ArgSourceArgs>("argSource", "menu.arg_source", "ArgSource",
                "Where a field's whole vocabulary comes from: a list known here, or the backend's completion of this line."),
            op::<m::LocaleArgs>("homeCatalogue", "menu.home_catalogue", "HomeRow[]", "The home page's catalogue, each sentence resolved by the locale."),
            op::<m::HomeLineArgs>("homeParse", "menu.home_parse", "Parsed",
                "Which command a home line is: nothing, none of this screen's, or a command and the rest of the line."),
            op::<m::FindArgs>("homeGripe", "menu.home_gripe", "Text", "The message refusing a project no workspace holds, naming the ones there are."),
        ]),
    ]
}
