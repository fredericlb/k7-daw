// ops/schema.rs — the schema.* operations: the small rules the page asks of a name, a ref, a clip or a key, each a pure function of its arguments.
// A reader would get wrong that a set the rule reads (the taken names) crosses as a list: a predicate cannot, and the answer is the same.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song::doc::{LevelSide, MixSide, own_level_side, own_mix_side};
use song::modules::types::{is_engine_ref, is_library_name, is_picture_pass, module_emits, module_status_of};
use song::modules::{
    BuiltinPlugin, Port, PortType, builtin_plugin, builtin_plugin_for_ref, engine_module_of, files::is_library_file_of,
};
use song_edit::playlist::{Playlist, parse_playlist, playlist_path_of};
use song_edit::sugar::{is_doc_file, strip_doc_ext};
use song_edit::takes::{CompSpan, set_comp_span};
use song_edit::tasks::{Task, parse_tasks};
use song_schema::{Key, Marker, beat_at_source, is_audio, source_at_beat, value_at};

/// A signal a module emits or a port carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Audio,
    Midi,
    Video,
}

impl From<PortType> for Signal {
    fn from(p: PortType) -> Signal {
        match p {
            PortType::Audio => Signal::Audio,
            PortType::Midi => Signal::Midi,
            PortType::Video => Signal::Video,
        }
    }
}

impl From<Signal> for PortType {
    fn from(s: Signal) -> PortType {
        match s {
            Signal::Audio => PortType::Audio,
            Signal::Midi => PortType::Midi,
            Signal::Video => PortType::Video,
        }
    }
}

/// A port as `moduleEmits` reads it: its signal alone.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OutPort {
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: Signal,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmitsArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub category: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub io: Option<String>,
    #[serde(default)]
    pub outputs: Vec<OutPort>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameArgs {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RefArgs {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub reference: String,
}

/// A module's `plugin:` as the builtin table reads it: its type and its name.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginArgs {
    #[serde(rename = "type", default)]
    #[cfg_attr(feature = "ts", ts(rename = "type", optional))]
    pub kind: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IdArgs {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StatusArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CategoryArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub category: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LibraryFileArgs {
    pub family: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EngineModuleArgs {
    pub name: String,
    pub format: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InstanceNameArgs {
    pub what: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub owner: Option<String>,
    pub taken: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValueAtArgs {
    pub keys: Vec<Key>,
    pub t: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BeatsAtSourcesArgs {
    pub markers: Vec<Marker>,
    pub secs: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SourcesAtBeatsArgs {
    pub markers: Vec<Marker>,
    pub beats: Vec<f64>,
}

/// One span of a comp, in song beats; `take` "" plays nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "CompSpan"))]
pub struct CompSpanDto {
    pub take: String,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SetCompSpanArgs {
    pub comp: Vec<CompSpanDto>,
    pub span: CompSpanDto,
    pub base_ref: String,
}

/// The module a scanned plugin becomes: its ref, and the plugin block that names it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EngineModule {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub reference: String,
    pub plugin: EnginePlugin,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnginePlugin {
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: String,
    pub name: String,
}

/// The timeline and lane an address names.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TimelineLane {
    pub timeline: String,
    pub lane: String,
}

/// One line of a playlist: the song, and one of its variants when it names one.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlaylistItem {
    pub song: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub variant: Option<String>,
}

/// A parsed playlist: `title` is what a listing shows, absent when the file says none.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "Playlist"))]
pub struct PlaylistDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub title: Option<String>,
    pub items: Vec<PlaylistItem>,
}

/// Where a task is about: a project folder and a position in it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TaskAt {
    pub project: String,
    pub timecode: String,
}

/// One task as read: its line in the file, so an edit can find it again.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "Task"))]
pub struct TaskDto {
    pub line: u32,
    pub text: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub at: Option<TaskAt>,
}

/// A built-in plugin's parameter: its span, and the named settings of a discrete one.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "BuiltinParam"))]
pub struct BuiltinParamDto {
    pub name: String,
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub def: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub states: Option<Vec<BuiltinStateDto>>,
    /// False for a control that is plain plugin state: no curve can drive it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
    pub automatable: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "BuiltinState"))]
pub struct BuiltinStateDto {
    pub label: String,
    pub value: f64,
}

/// A built-in plugin as the picker and the builder know it: display name, role, parameters.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "BuiltinPluginInfo"))]
pub struct BuiltinPluginDto {
    pub name: String,
    pub role: String,
    pub params: Vec<BuiltinParamDto>,
}

/// The schema family's requests: `{"op": "schema.is_doc_file", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum SchemaRequest {
    #[serde(rename = "schema.module_emits")]
    ModuleEmits(EmitsArgs),
    #[serde(rename = "schema.module_status_of")]
    ModuleStatusOf(StatusArgs),
    #[serde(rename = "schema.is_picture_pass")]
    IsPicturePass(CategoryArgs),
    #[serde(rename = "schema.is_engine_ref")]
    IsEngineRef(RefArgs),
    #[serde(rename = "schema.is_library_name")]
    IsLibraryName(NameArgs),
    #[serde(rename = "schema.is_library_file_of")]
    IsLibraryFileOf(LibraryFileArgs),
    #[serde(rename = "schema.own_level_side")]
    OwnLevelSide(NameArgs),
    #[serde(rename = "schema.own_mix_side")]
    OwnMixSide(NameArgs),
    #[serde(rename = "schema.engine_module_of")]
    EngineModuleOf(EngineModuleArgs),
    #[serde(rename = "schema.builtin_for_ref")]
    BuiltinForRef(RefArgs),
    #[serde(rename = "schema.builtin_for_plugin")]
    BuiltinForPlugin(PluginArgs),
    #[serde(rename = "schema.is_preset_name")]
    IsPresetName(NameArgs),
    #[serde(rename = "schema.placement_start_of")]
    PlacementStartOf(IdArgs),
    #[serde(rename = "schema.is_doc_file")]
    IsDocFile(NameArgs),
    #[serde(rename = "schema.strip_doc_ext")]
    StripDocExt(NameArgs),
    #[serde(rename = "schema.is_audio")]
    IsAudio(NameArgs),
    #[serde(rename = "schema.instance_name")]
    InstanceName(InstanceNameArgs),
    #[serde(rename = "schema.value_at")]
    ValueAt(ValueAtArgs),
    #[serde(rename = "schema.material_ref_of")]
    MaterialRefOf(IdArgs),
    #[serde(rename = "schema.timeline_lane_of")]
    TimelineLaneOf(IdArgs),
    #[serde(rename = "schema.parse_playlist")]
    ParsePlaylist(TextArgs),
    #[serde(rename = "schema.playlist_path_of")]
    PlaylistPathOf(NameArgs),
    #[serde(rename = "schema.parse_tasks")]
    ParseTasks(TextArgs),
    #[serde(rename = "schema.set_comp_span")]
    SetCompSpan(SetCompSpanArgs),
    #[serde(rename = "schema.beats_at_sources")]
    BeatsAtSources(BeatsAtSourcesArgs),
    #[serde(rename = "schema.sources_at_beats")]
    SourcesAtBeats(SourcesAtBeatsArgs),
}

fn emits(a: &EmitsArgs) -> Option<Signal> {
    let outputs: Vec<Port> = a
        .outputs
        .iter()
        .map(|o| Port {
            name: String::new(),
            kind: o.kind.into(),
            mandatory: None,
        })
        .collect();
    module_emits(a.category.as_deref(), a.io.as_deref(), &outputs).map(Signal::from)
}

fn engine_module(a: &EngineModuleArgs) -> Option<EngineModule> {
    let (reference, plugin) = engine_module_of(&a.name, &a.format)?;
    Some(EngineModule {
        reference,
        plugin: EnginePlugin {
            kind: plugin.plugin_type.unwrap_or_default(),
            name: plugin.name.unwrap_or_default(),
        },
    })
}

fn builtin_for_ref(reference: &str) -> Option<BuiltinPluginDto> {
    builtin_plugin_for_ref(reference).map(builtin_dto)
}

fn builtin_dto(p: &BuiltinPlugin) -> BuiltinPluginDto {
    BuiltinPluginDto {
        name: p.name.clone(),
        role: p.role.clone(),
        params: p
            .params
            .iter()
            .map(|q| BuiltinParamDto {
                name: q.name.clone(),
                min: q.min,
                max: q.max,
                def: q.def,
                states: q.states.as_ref().map(|s| {
                    s.iter()
                        .map(|x| BuiltinStateDto {
                            label: x.label.clone(),
                            value: x.value,
                        })
                        .collect()
                }),
                automatable: q.automatable,
            })
            .collect(),
    }
}

fn playlist(p: Playlist) -> PlaylistDto {
    PlaylistDto {
        title: p.title,
        items: p
            .items
            .into_iter()
            .map(|i| PlaylistItem {
                song: i.song,
                variant: i.variant,
            })
            .collect(),
    }
}

fn task(t: Task) -> TaskDto {
    TaskDto {
        line: u32::try_from(t.line).unwrap_or(u32::MAX),
        text: t.text,
        done: t.done,
        at: t.at.map(|(project, timecode)| TaskAt { project, timecode }),
    }
}

fn span(s: CompSpanDto) -> CompSpan {
    CompSpan {
        take: s.take,
        from: s.from,
        to: s.to,
    }
}

fn dto(s: CompSpan) -> CompSpanDto {
    CompSpanDto {
        take: s.take,
        from: s.from,
        to: s.to,
    }
}

pub fn dispatch(request: SchemaRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use SchemaRequest as R;
    match request {
        R::ModuleEmits(a) => write(out, &emits(&a)),
        R::ModuleStatusOf(a) => write(out, module_status_of(a.status.as_deref())),
        R::IsPicturePass(a) => write(out, &is_picture_pass(a.category.as_deref())),
        R::IsEngineRef(a) => write(out, &is_engine_ref(&a.reference)),
        R::IsLibraryName(a) => write(out, &is_library_name(&a.name)),
        R::IsLibraryFileOf(a) => write(out, &is_library_file_of(&a.family, &a.path)),
        R::OwnLevelSide(a) => write(
            out,
            &own_level_side(&a.name).map(|s| match s {
                LevelSide::Input => "input",
                LevelSide::Output => "output",
            }),
        ),
        R::OwnMixSide(a) => write(
            out,
            &own_mix_side(&a.name).map(|s| match s {
                MixSide::Wet => "wet",
                MixSide::Dry => "dry",
            }),
        ),
        R::EngineModuleOf(a) => write(out, &engine_module(&a)),
        R::BuiltinForRef(a) => write(out, &builtin_for_ref(&a.reference)),
        R::BuiltinForPlugin(a) => write(
            out,
            &builtin_plugin(a.kind.as_deref(), a.name.as_deref()).map(builtin_dto),
        ),
        R::IsPresetName(a) => write(out, &song_edit::presets::is_preset_name(&a.name)),
        R::PlacementStartOf(a) => write(out, &song::clips::placement_start_of(&a.id)),
        R::IsDocFile(a) => write(out, &is_doc_file(&a.name)),
        R::StripDocExt(a) => write(out, strip_doc_ext(&a.name)),
        R::IsAudio(a) => write(out, &is_audio(&a.name)),
        R::InstanceName(a) => {
            let taken = |n: &str| a.taken.iter().any(|t| t == n);
            write(
                out,
                &song_edit::instance_name::instance_name(&a.what, a.owner.as_deref(), &taken),
            )
        }
        R::ValueAt(a) => write(out, &value_at(&a.keys, a.t)),
        R::MaterialRefOf(a) => write(out, song::clips::material_ref_of(&a.id)),
        R::TimelineLaneOf(a) => write(
            out,
            &song_edit::chain_item_edit::timeline_lane_of(&a.id).map(|(timeline, lane)| TimelineLane {
                timeline: timeline.to_owned(),
                lane: lane.to_owned(),
            }),
        ),
        R::ParsePlaylist(a) => write(out, &parse_playlist(&a.text).map(playlist)),
        R::PlaylistPathOf(a) => write(out, &playlist_path_of(&a.name)),
        R::ParseTasks(a) => write(out, &parse_tasks(&a.text).into_iter().map(task).collect::<Vec<_>>()),
        R::SetCompSpan(a) => {
            let comp: Vec<CompSpan> = a.comp.into_iter().map(span).collect();
            let laid = set_comp_span(&comp, &span(a.span), &a.base_ref);
            write(out, &laid.into_iter().map(dto).collect::<Vec<_>>())
        }
        R::BeatsAtSources(a) => write(
            out,
            &a.secs
                .iter()
                .map(|s| beat_at_source(&a.markers, *s))
                .collect::<Vec<_>>(),
        ),
        R::SourcesAtBeats(a) => write(
            out,
            &a.beats
                .iter()
                .map(|b| source_at_beat(&a.markers, *b))
                .collect::<Vec<_>>(),
        ),
    }
}
