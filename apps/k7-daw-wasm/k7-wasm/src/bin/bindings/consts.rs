// consts.rs — the text of consts.generated.ts: the values the page reads from the crates, written as TypeScript literals.
// A reader would get wrong that ts-rs writes these: it emits types only, so each table is printed here from the Rust constant.

use serde_json::{Value, json};
use song::doc::{TRACK_CATEGORIES, TRACK_PREFIXES};
use song::modules::MODULE_TYPES;
use song::modules::names as n;
use song::modules::types::{IO_MODULES, IO_SOURCES};
use song::views::surfaces::SURFACE_TYPES;
use song_schema::{
    ASPECTS, EXPORT_FORMATS, EXPORT_QUALITIES, STEM_NEUTRAL, STEM_RANGE, STEMS, STEMS_DIR, Stem, TICKS_PER_QUARTER,
    TIMECODE_PATTERN,
};

/// A value as a TypeScript literal: JSON is one.
fn lit(v: &Value) -> String {
    v.to_string()
}

fn ports(list: &[(&str, song::modules::PortType, Option<bool>)]) -> Value {
    Value::Array(
        list.iter()
            .map(|(name, ty, mandatory)| {
                let mut p = json!({ "name": name, "type": ty });
                if let Some(m) = mandatory {
                    p["mandatory"] = json!(m);
                }
                p
            })
            .collect(),
    )
}

fn module_types() -> Value {
    let mut map = serde_json::Map::new();
    for t in &MODULE_TYPES {
        map.insert(
            t.name.to_owned(),
            json!({ "kind": t.kind, "inputs": ports(t.inputs), "outputs": ports(t.outputs) }),
        );
    }
    Value::Object(map)
}

fn io_modules() -> Value {
    Value::Object(
        IO_MODULES
            .iter()
            .map(|(n, io)| ((*n).to_owned(), json!({ "io": io })))
            .collect(),
    )
}

/// A table of string lists as an object, in the Rust table's order.
fn families(list: &[(&str, &[&str])]) -> Value {
    Value::Object(list.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect())
}

/// A table of string pairs as an object.
fn pairs(list: &[(&str, &str)]) -> Value {
    Value::Object(list.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect())
}

/// One exported constant: its doc line, its name and its value, `as const` for a table.
fn export(doc: &str, name: &str, value: &Value) -> [String; 3] {
    // A scalar stays a plain const, whose literal type widens in a `Set` or an array; a table is `as const`.
    let frozen = if value.is_array() || value.is_object() {
        " as const"
    } else {
        ""
    };
    [
        format!("/** {doc} */"),
        format!("export const {name} = {}{frozen};", lit(value)),
        String::new(),
    ]
}

/// The constants that are plain values: names of nodes, type folders, params, library folders and files.
fn plain() -> Vec<[String; 3]> {
    let s = |v: &str| json!(v);
    vec![
        export(
            "The sum the mixer makes, and the device that plays it.",
            "MIX_NODE",
            &s(n::MIX_NODE),
        ),
        export("The interface's output node.", "OUT_NODE", &s(n::OUT_NODE)),
        export(
            "The picture's render node, on a project that declares a viewport.",
            "RENDER_NODE",
            &s(n::RENDER_NODE),
        ),
        export("The picture's output device.", "VIDEO_OUT_NODE", &s(n::VIDEO_OUT_NODE)),
        export(
            "The headphone bus: the one output that leaves the mixer's sum entirely.",
            "CUE_NODE",
            &s(n::CUE_NODE),
        ),
        export(
            "An audio input's node: a channel of the interface.",
            "AUDIO_IN_NODE",
            &s(n::AUDIO_IN_NODE),
        ),
        export("A MIDI device's node.", "MIDI_IN_NODE", &s(n::MIDI_IN_NODE)),
        export("A camera's node.", "CAMERA_NODE", &s(n::CAMERA_NODE)),
        export("The io kind a live picture source is.", "CAMERA_IO", &s(n::CAMERA_IO)),
        export("The io kind a MIDI device is.", "MIDI_IN_IO", &s(n::MIDI_IN_IO)),
        export(
            "The owner a master chain's plugins are filed under while the document is built.",
            "MASTER_OWNER",
            &s(n::MASTER_OWNER),
        ),
        export(
            "The type folder cold modules are filed in.",
            "COLD_TYPE",
            &s(n::COLD_TYPE),
        ),
        export(
            "The type folder a console's halves live in.",
            "CONSOLE_TYPE",
            &s(n::CONSOLE_TYPE),
        ),
        export(
            "The type folder a lane belongs to.",
            "TRACK_CATEGORY",
            &s(n::TRACK_CATEGORY),
        ),
        export(
            "Where a rack keeps the audio its symbols name, inside its own folder.",
            "MODULE_SOURCES_DIR",
            &s(n::MODULE_SOURCES_DIR),
        ),
        export("A shader module's folder of GLSL.", "SHADERS_DIR", &s(n::SHADERS_DIR)),
        export("A shader's vertex file.", "SHADER_VERTEX", &s(n::SHADER_VERTEX)),
        export("A shader's fragment file.", "SHADER_FRAGMENT", &s(n::SHADER_FRAGMENT)),
        export(
            "The type folder a shader module lives in.",
            "SHADER_CATEGORY",
            &s(n::SHADER_CATEGORY),
        ),
        export(
            "The type folder a transition module lives in.",
            "TRANSITION_CATEGORY",
            &s(n::TRANSITION_CATEGORY),
        ),
        export(
            "The type folder a shape module lives in.",
            "SHAPE_CATEGORY",
            &s(n::SHAPE_CATEGORY),
        ),
        export(
            "What a shape module draws, named by its `shape:` field.",
            "SHAPE_KINDS",
            &json!(n::SHAPE_KINDS),
        ),
        export(
            "The four knobs every shader gets, numbered and 0..1.",
            "SHADER_PARAMS",
            &json!(n::SHADER_PARAMS),
        ),
        export("The kind a bare `fade_in` means.", "DISSOLVE", &s(n::DISSOLVE)),
        export(
            "The kind a fade naming a `chain` and no `with` means.",
            "OVER",
            &s(n::OVER),
        ),
        export("The picture lane's strip.", "PICTURE_STRIP", &s(n::PICTURE_STRIP)),
        export(
            "The strip's shader name, which the compiled scene still carries.",
            "PICTURE_STRIP_SHADER",
            &s(n::PICTURE_STRIP_SHADER),
        ),
        export("How a chain names an engine module.", "ENGINE_NS", &s(n::ENGINE_NS)),
        export(
            "How a chain names a built-in: the app's own vocabulary.",
            "META_NS",
            &s(n::META_NS),
        ),
        export("The doc module's type folder.", "DOC_CATEGORY", &s(n::DOC_CATEGORY)),
        export(
            "The module every note on the graph is a node of.",
            "DOC_REF",
            &s(n::DOC_REF),
        ),
        export(
            "What a node stands on when its plugin is one k7 cannot load.",
            "NOT_SUPPORTED_REF",
            &s(n::NOT_SUPPORTED_REF),
        ),
        export(
            "The not-supported module's category: no type folder.",
            "NOT_SUPPORTED_CATEGORY",
            &s(n::NOT_SUPPORTED_CATEGORY),
        ),
        export(
            "`plugin: { type: not_supported }`: the marker for a plugin k7 knows and cannot load.",
            "NOT_SUPPORTED_TYPE",
            &s(n::NOT_SUPPORTED_TYPE),
        ),
        export(
            "`plugin: { type: offline }`: no engine plugin, a pass onto disk first.",
            "OFFLINE_TYPE",
            &s(n::OFFLINE_TYPE),
        ),
        export(
            "The modules a lane is a node of: sound, strudel, group, picture.",
            "TRACK_REFS",
            &json!(n::TRACK_REFS),
        ),
        export(
            "How a module and its node are keyed: `module:node`.",
            "NODE_SEP",
            &json!(n::NODE_SEP.to_string()),
        ),
        export(
            "How a module's status asks to be treated by the completions.",
            "MODULE_STATUSES",
            &json!(n::MODULE_STATUSES),
        ),
        export(
            "The intrinsic a module's engine-side dry/wet is bound by.",
            "MIX_PARAM",
            &s(n::MIX_PARAM),
        ),
        export(
            "The level into an audio module, in dB.",
            "INPUT_VOLUME_PARAM",
            &s(n::INPUT_VOLUME_PARAM),
        ),
        export(
            "The level out of anything with audio out, in dB.",
            "OUTPUT_VOLUME_PARAM",
            &s(n::OUTPUT_VOLUME_PARAM),
        ),
        export(
            "The range both levels take, in dB.",
            "VOLUME_RANGE",
            &json!({ "min": n::VOLUME_RANGE.0, "max": n::VOLUME_RANGE.1, "def": n::VOLUME_RANGE.2 }),
        ),
        export(
            "A rack's first pad note: C1, General MIDI's kick.",
            "RACK_FIRST_NOTE",
            &json!(n::RACK_FIRST_NOTE),
        ),
        export(
            "The two fixed levels a module can live at.",
            "FIXED_NAMESPACES",
            &json!(n::FIXED_NAMESPACES),
        ),
        export(
            "The library an account's first repository is named.",
            "DEFAULT_LIBRARY",
            &s(n::DEFAULT_LIBRARY),
        ),
        export(
            "The account's own library, `@account/…`.",
            "ACCOUNT_LIBRARY",
            &s(n::ACCOUNT_LIBRARY),
        ),
        export(
            "The folder the account's libraries answer to in a workspace's file API.",
            "LIBRARIES_DIR",
            &s(n::LIBRARIES_DIR),
        ),
        export(
            "Where the file service clones the libraries, under the machine's state.",
            "LIBRARIES_HOME_DIR",
            &s(n::LIBRARIES_HOME_DIR),
        ),
        export(
            "Where automatic VST renders are cached, under the machine's state.",
            "AUTO_RENDERS_HOME_DIR",
            &s(n::AUTO_RENDERS_HOME_DIR),
        ),
        export(
            "Where every project's undo journal is kept, under the machine's state.",
            "HISTORY_HOME_DIR",
            &s(n::HISTORY_HOME_DIR),
        ),
        export(
            "The folder a workspace's own shared library sits in.",
            "SHARED_MODULES_DIR",
            &s(n::SHARED_MODULES_DIR),
        ),
        export(
            "The params whose value is a file the plugin loads, not a number.",
            "FILE_PARAMS",
            &json!(n::FILE_PARAMS),
        ),
        export(
            "File params whose file is in a library's shared folder.",
            "LIBRARY_FILE_PARAMS",
            &pairs(&n::LIBRARY_FILE_PARAMS),
        ),
        export(
            "File params a module may fill from its own `assets/` or a library's shared folder.",
            "SHARED_FILE_PARAMS",
            &pairs(&n::SHARED_FILE_PARAMS),
        ),
        export(
            "The library folders that hold files rather than modules, and what each is walked for.",
            "LIBRARY_FILE_FAMILIES",
            &families(&n::LIBRARY_FILE_FAMILIES),
        ),
        export(
            "What a rack tap's sample picker asks for.",
            "TAP_SAMPLE",
            &s(n::TAP_SAMPLE),
        ),
        export(
            "What separates a timeline from its lane, and the timelines a nested clip's id walks through.",
            "NEST_SEP",
            &s(n::NEST_SEP),
        ),
        export(
            "The placement fields a timeline event may not carry.",
            "FIXED_FIELDS",
            &json!(n::FIXED_FIELDS),
        ),
        export(
            "The word for the project's own timeline.",
            "SONG_TARGET",
            &s(n::SONG_TARGET),
        ),
        export(
            "Where the workspace's tasks live.",
            "TASKS_FILE",
            &s(song_edit::tasks::TASKS_FILE),
        ),
        export(
            "The folder a workspace keeps its playlists in.",
            "PLAYLISTS_DIR",
            &s(song_edit::playlist::PLAYLISTS_DIR),
        ),
        export(
            "The four sources a separation produces, IN THE ORDER the model returns them.",
            "STEMS",
            &json!(STEMS.map(Stem::word)),
        ),
        export(
            "Where a separation's outputs go: beside the file, never next to it.",
            "STEMS_DIR",
            &s(STEMS_DIR),
        ),
        export(
            "A stem knob's travel: 0 is the part gone, 1 is as recorded, 2 is twice as much.",
            "STEM_RANGE",
            &json!(STEM_RANGE),
        ),
        export(
            "The middle of that travel, and the record as it was cut.",
            "STEM_NEUTRAL",
            &json!(STEM_NEUTRAL),
        ),
        export(
            "The frame shapes on offer, and the pixels each means.",
            "ASPECTS",
            &Value::Array(
                ASPECTS
                    .iter()
                    .map(|a| json!({ "name": a.name, "w": a.w, "h": a.h }))
                    .collect(),
            ),
        ),
        export(
            "The quality presets an export may ask for, lowest first.",
            "EXPORT_QUALITIES",
            &json!(EXPORT_QUALITIES),
        ),
        export(
            "The containers an export leaves in, the default first.",
            "EXPORT_FORMATS",
            &json!(EXPORT_FORMATS),
        ),
        export(
            "The containers the picture leaves in, each one an export format.",
            "VIDEO_FORMATS",
            &json!(EXPORT_FORMATS),
        ),
        export(
            "The kinds of surface a project may declare.",
            "SURFACE_TYPES",
            &json!(SURFACE_TYPES),
        ),
        export(
            "The track categories, in the order their tracks are laid out.",
            "TRACK_CATEGORIES",
            &json!(TRACK_CATEGORIES),
        ),
        export(
            "The instrument codes a track name may lead with: each one's family and the label a row shows.",
            "TRACK_PREFIXES",
            &Value::Object(
                TRACK_PREFIXES
                    .iter()
                    .map(|(p, f, l)| ((*p).to_owned(), json!([f.name(), l])))
                    .collect(),
            ),
        ),
    ]
}

/// The constants of the command bar and the menus.
#[cfg(feature = "verbs")]
fn ui_consts() -> Vec<[String; 3]> {
    vec![
        export(
            "What a right-click can be on, as commands.yaml `menu:` names it.",
            "MENU_ENTITIES",
            &json!(verbs::palette::MENU_ENTITIES),
        ),
        export(
            "How many commands the bar remembers: the recency bonus decays across them.",
            "MAX_RECENT",
            &json!(verbs::ui::rank::MAX_RECENT),
        ),
        export(
            "How many recent commands the empty bar offers.",
            "EMPTY_RECENT_MAX",
            &json!(verbs::ui::rank::EMPTY_RECENT_MAX),
        ),
        export(
            "The mixdown formats the file service renders.",
            "MIX_FORMATS",
            &json!(verbs::ui::export::MIX_FORMATS),
        ),
        export(
            "The containers the picture leaves in, the default first.",
            "VIDEO_FORMATS",
            &json!(verbs::ui::export::VIDEO_FORMATS),
        ),
        export(
            "The cut handed to another editor: OpenTimelineIO, or a CMX 3600 EDL of one lane.",
            "TIMELINE_FORMATS",
            &json!(verbs::ui::export::TIMELINE_FORMATS),
        ),
        export(
            "What the audio of an exported video is rendered from when the line names none.",
            "DEFAULT_VIDEO_AUDIO",
            &json!(verbs::ui::export::DEFAULT_VIDEO_AUDIO),
        ),
        export(
            "The usage line of `project export`, built from the vocabularies above.",
            "EXPORT_USAGE",
            &json!(verbs::ui::export::usage()),
        ),
    ]
}

/// The whole file; biome formats it after it is written.
pub fn text() -> String {
    let port = "{ name: string; type: \"audio\" | \"midi\" | \"video\"; mandatory?: boolean }";
    let module_type =
        format!("{{ kind: \"io\" | \"plugin\" | \"doc\" | \"track\"; inputs: {port}[]; outputs: {port}[] }}");
    let io = "{ io: \"in\" | \"out\" | \"file\" | \"camera\" | \"midi\" }";
    let mut lines: Vec<String> = [
        "// consts.generated.ts — GENERATED by apps/k7-daw-wasm/k7-wasm/src/bin/bindings/consts.rs from the crates' constants. Do not edit.".to_owned(),
        "// Run `npm run build:wasm -w @studio/k7-wasm` after changing a constant there.".to_owned(),
        String::new(),
        "/** Ticks per quarter note. */".to_owned(),
        format!("export const TICKS_PER_QUARTER = {TICKS_PER_QUARTER};"),
        String::new(),
        "/** The shape a position or a duration is written in. */".to_owned(),
        format!("export const TIMECODE = /{TIMECODE_PATTERN}/;"),
        String::new(),
        "/** The io roles the signal comes FROM. */".to_owned(),
        format!("export const IO_SOURCES = {} as const;", lit(&json!(IO_SOURCES))),
        String::new(),
        "/** What a module of each type folder has when it says nothing itself: its default ports. */".to_owned(),
        format!("export const MODULE_TYPES: Record<string, {module_type}> = {};", lit(&module_types())),
        String::new(),
        "/** The io roles the app itself provides, by node name. */".to_owned(),
        format!("export const IO_MODULES: Record<string, {io}> = {};", lit(&io_modules())),
        String::new(),
    ]
    .to_vec();
    lines.extend(plain().into_iter().flatten());
    #[cfg(feature = "verbs")]
    lines.extend(ui_consts().into_iter().flatten());
    lines.join("\n")
}
