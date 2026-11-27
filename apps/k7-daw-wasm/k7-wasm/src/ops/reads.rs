// ops/reads.rs — the reads.* operations: what the page reads off the open project's document, each a function of the document it passes.
// A reader would get wrong that an answer names rows and pieces by INDEX into what was passed: the page keeps its objects, Rust the rule.

use super::text::TextArgs;
use super::text_desk::chain_read;
use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use serde_json::{Value, json};
use song::reads::{self, RClip, RGraph, RGroup, RNote, RRack, RTrack, RVideoTrack};
use song_edit::project_root::{MASTER_MAX_DB, MASTER_MIN_DB};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopArgs {
    pub kind: String,
    pub length: f64,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TracksArgs {
    pub tracks: Vec<RTrack>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NoteSpansArgs {
    pub tracks: Vec<RTrack>,
    pub tempo: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VideoArgs {
    pub tracks: Vec<RVideoTrack>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RawArgs {
    pub raw: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AudioActivityArgs {
    pub peak: Vec<f64>,
    pub buckets: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiActivityArgs {
    pub notes: Vec<RNote>,
    pub span_beats: f64,
    pub buckets: usize,
    pub loop_start: f64,
    pub loop_end: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AlphasArgs {
    pub alphas: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakesEventArgs {
    pub track: String,
    pub event_id: String,
    pub clips: Vec<RClip>,
    pub num: f64,
    pub den: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Span3 {
    pub take: String,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeardArgs {
    pub comp: Vec<Span3>,
    pub own: bool,
    #[serde(rename = "ref")]
    pub ref_: String,
    pub start: f64,
    pub row_end: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Span2 {
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SeamsArgs {
    pub start: f64,
    pub to: f64,
    pub comp: Vec<Span2>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Meter {
    pub num: f64,
    pub den: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompLineArgs {
    pub track: String,
    pub at: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    pub from: f64,
    pub to: f64,
    #[serde(flatten)]
    pub meter: Meter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ArmLineArgs {
    pub track: String,
    pub at: String,
    pub from: f64,
    pub to: Option<f64>,
    #[serde(flatten)]
    pub meter: Meter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FadeLineArgs {
    pub track: String,
    pub at: String,
    pub seam: f64,
    pub beats: f64,
    #[serde(flatten)]
    pub meter: Meter,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeLinesArgs {
    pub track: String,
    pub at: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    pub pieces: Vec<RClip>,
    pub piece_id: String,
    pub from: f64,
    pub to: f64,
    #[serde(flatten)]
    pub meter: Meter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RowsArgs {
    pub groups: Vec<String>,
    pub tracks: Vec<RTrack>,
    pub video: Vec<String>,
    pub automation: HashMap<String, usize>,
    pub order: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlacingArgs {
    pub tracks: Vec<RTrack>,
    pub clip: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlacementArgs {
    pub clips: Vec<RClip>,
    #[serde(rename = "ref")]
    pub ref_: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GraphTrackArgs {
    pub graph: RGraph,
    pub track: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GraphArgs {
    pub graph: RGraph,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RackArgs {
    pub rack: Vec<RRack>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WithoutNotesArgs {
    pub graph: Value,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TemplateArgs {
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LineArgs {
    pub line: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RackHitsArgs {
    pub tracks: Vec<RTrack>,
    pub graph: RGraph,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RefArgs {
    #[serde(rename = "ref")]
    pub ref_: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedMasterArgs {
    pub master_db: f64,
    /// A meter that read silence is `-Infinity`, which JSON writes as null.
    pub peak_db: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimArgs {
    pub peak_db: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TapArgs {
    pub taps: Vec<f64>,
    pub now: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FolderArgs {
    pub raw: String,
    pub suffixes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FollowArgs {
    pub open: Option<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SurfacesArgs {
    pub surfaces: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnginePluginArgs {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub name: Option<String>,
    pub fallback: String,
    pub emits: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellArgs {
    pub tempo: f64,
    pub num: Option<f64>,
    pub den: Option<f64>,
    pub count_in: f64,
    pub tracks: Vec<RTrack>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LaneMixArgs {
    pub tracks: Vec<RTrack>,
    pub groups: Vec<RGroup>,
    pub video: Vec<RVideoTrack>,
}

/// The reads family's requests: `{"op": "reads.clip_loop", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", content = "args")]
pub enum ReadsRequest {
    #[serde(rename = "reads.clip_loop")]
    ClipLoop(LoopArgs),
    #[serde(rename = "reads.played_zone")]
    PlayedZone(LoopArgs),
    #[serde(rename = "reads.material_instances")]
    MaterialInstances(TracksArgs),
    #[serde(rename = "reads.mixer_modules")]
    MixerModules(TracksArgs),
    #[serde(rename = "reads.note_spans")]
    NoteSpans(NoteSpansArgs),
    #[serde(rename = "reads.video_spans")]
    VideoSpans(VideoArgs),
    #[serde(rename = "reads.to_alphas")]
    ToAlphas(RawArgs),
    #[serde(rename = "reads.audio_activity")]
    AudioActivity(AudioActivityArgs),
    #[serde(rename = "reads.midi_activity")]
    MidiActivity(MidiActivityArgs),
    #[serde(rename = "reads.activity_mask")]
    ActivityMask(AlphasArgs),
    #[serde(rename = "reads.takes_event")]
    TakesEvent(TakesEventArgs),
    #[serde(rename = "reads.heard_spans")]
    HeardSpans(HeardArgs),
    #[serde(rename = "reads.comp_seams")]
    CompSeams(SeamsArgs),
    #[serde(rename = "reads.comp_line")]
    CompLine(CompLineArgs),
    #[serde(rename = "reads.arm_line")]
    ArmLine(ArmLineArgs),
    #[serde(rename = "reads.comp_fade_line")]
    CompFadeLine(FadeLineArgs),
    #[serde(rename = "reads.piece_edge_lines")]
    PieceEdgeLines(EdgeLinesArgs),
    #[serde(rename = "reads.rows")]
    Rows(RowsArgs),
    #[serde(rename = "reads.track_placing")]
    TrackPlacing(PlacingArgs),
    #[serde(rename = "reads.placement_of")]
    PlacementOf(PlacementArgs),
    #[serde(rename = "reads.midi_in_device_for")]
    MidiInDeviceFor(GraphTrackArgs),
    #[serde(rename = "reads.midi_in_lanes")]
    MidiInLanes(GraphArgs),
    #[serde(rename = "reads.midi_in_played")]
    MidiInPlayed(GraphArgs),
    #[serde(rename = "reads.lane_input")]
    LaneInput(GraphTrackArgs),
    #[serde(rename = "reads.rack_for")]
    RackFor(GraphTrackArgs),
    #[serde(rename = "reads.rack_key_names")]
    RackKeyNames(RackArgs),
    #[serde(rename = "reads.without_notes")]
    WithoutNotes(WithoutNotesArgs),
    #[serde(rename = "reads.template_graph")]
    TemplateGraph(TemplateArgs),
    #[serde(rename = "reads.gives_channel")]
    GivesChannel(LineArgs),
    #[serde(rename = "reads.has_audible_input")]
    HasAudibleInput(GraphArgs),
    #[serde(rename = "reads.rack_hits")]
    RackHits(RackHitsArgs),
    #[serde(rename = "reads.clip_name_parts")]
    ClipNameParts(RefArgs),
    #[serde(rename = "reads.fixed_master")]
    FixedMaster(FixedMasterArgs),
    #[serde(rename = "reads.shell_view")]
    ShellView(ShellArgs),
    #[serde(rename = "reads.lane_mix")]
    LaneMix(LaneMixArgs),
    #[serde(rename = "reads.blanks")]
    Blanks,
    #[serde(rename = "reads.engine_plugin")]
    EnginePlugin(EnginePluginArgs),
    #[serde(rename = "reads.trim_for")]
    TrimFor(TrimArgs),
    #[serde(rename = "reads.tap_tempo")]
    TapTempo(TapArgs),
    #[serde(rename = "reads.folder_name")]
    FolderName(FolderArgs),
    #[serde(rename = "reads.follow_rename")]
    FollowRename(FollowArgs),
    #[serde(rename = "reads.shape_of_surfaces")]
    ShapeOfSurfaces(SurfacesArgs),
}

fn bad(e: String) -> WasmError {
    WasmError::bad_request(e)
}

/// The chain file's text as a one-chain graph, or the first reason it does not read.
fn template_graph(a: &TemplateArgs) -> Value {
    let read = chain_read(&TextArgs { text: a.text.clone() });
    if !read.issues.is_empty() {
        let msg: Vec<String> = read
            .issues
            .iter()
            .map(|i| {
                let line = i.line.filter(|l| *l > 0).map(|l| format!("{l}: ")).unwrap_or_default();
                let path = if i.path.is_empty() { String::new() } else { format!("{}: ", i.path) };
                format!("{line}{path}{}", i.message)
            })
            .collect();
        return json!({ "error": msg.join("; ") });
    }
    let steps: Vec<Value> = read
        .items
        .iter()
        .map(|item| {
            let mut s = serde_json::Map::new();
            s.insert("module".into(), json!(item.id));
            if let Some(e) = item.extends.as_ref().filter(|e| !e.is_empty()) {
                s.insert("uses".into(), json!(e));
            }
            if item.bypass {
                s.insert("enabled".into(), json!(false));
            }
            Value::Object(s)
        })
        .collect();
    json!({ "graph": {
        "chains": { a.name.clone(): { "from": "@in", "use": steps, "out": [{ "to": "@out", "gain": 1 }] } },
        "tracks": [],
        "warnings": [],
    } })
}

pub fn dispatch(request: ReadsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use ReadsRequest as R;
    match request {
        R::ClipLoop(a) => write(out, &reads::clip_loop(&a.kind, a.length, a.loop_start, a.loop_end)),
        R::PlayedZone(a) => write(
            out,
            &reads::played_zone(&a.kind, a.length, a.loop_start, a.loop_end).map(|(from, to)| json!({ "from": from, "to": to })),
        ),
        R::MaterialInstances(a) => write(out, &reads::material_instances(&a.tracks)),
        R::MixerModules(a) => write(out, &reads::mixer_modules(&a.tracks)),
        R::NoteSpans(a) => write(out, &reads::note_spans(&a.tracks, a.tempo)),
        R::VideoSpans(a) => write(out, &reads::video_spans(&a.tracks)),
        R::ToAlphas(a) => write(out, &reads::to_alphas(&a.raw)),
        R::AudioActivity(a) => write(out, &reads::audio_activity(&a.peak, a.buckets)),
        R::MidiActivity(a) => write(out, &reads::midi_activity(&a.notes, a.span_beats, a.buckets, a.loop_start, a.loop_end)),
        R::ActivityMask(a) => write(out, &reads::activity_mask(&a.alphas)),
        R::TakesEvent(a) => write(out, &reads::takes_event_of(&a.track, &a.event_id, &a.clips, a.num, a.den).map_err(bad)?),
        R::HeardSpans(a) => {
            let comp: Vec<(String, f64, f64)> = a.comp.into_iter().map(|s| (s.take, s.from, s.to)).collect();
            write(out, &reads::heard_spans(&comp, a.own, &a.ref_, a.start, a.row_end).into_iter().map(|(from, to)| json!({ "from": from, "to": to })).collect::<Vec<_>>())
        }
        R::CompSeams(a) => {
            let comp: Vec<(f64, f64)> = a.comp.into_iter().map(|s| (s.from, s.to)).collect();
            write(out, &reads::comp_seams(a.start, a.to, &comp))
        }
        R::CompLine(a) => write(out, &reads::comp_line(&a.track, &a.at, &a.ref_, a.from, a.to, a.meter.num, a.meter.den).map_err(bad)?),
        R::ArmLine(a) => write(out, &reads::arm_line(&a.track, &a.at, a.from, a.to, a.meter.num, a.meter.den).map_err(bad)?),
        R::CompFadeLine(a) => write(out, &reads::comp_fade_line(&a.track, &a.at, a.seam, a.beats, a.meter.num, a.meter.den).map_err(bad)?),
        R::PieceEdgeLines(a) => write(
            out,
            &reads::piece_edge_lines(&a.track, &a.at, &a.ref_, &a.pieces, &a.piece_id, a.from, a.to, a.meter.num, a.meter.den)
                .map_err(bad)?,
        ),
        R::Rows(a) => write(out, &reads::project_rows(&a.groups, &a.tracks, &a.video, &a.automation, a.order.as_deref())),
        R::TrackPlacing(a) => write(out, &reads::track_placing(&a.tracks, &a.clip)),
        R::PlacementOf(a) => write(out, &reads::placement_of(&a.clips, &a.ref_)),
        R::MidiInDeviceFor(a) => write(out, &reads::midi_in_device_for(&a.graph, &a.track)),
        R::MidiInLanes(a) => write(out, &reads::midi_in_lanes(&a.graph)),
        R::MidiInPlayed(a) => write(out, &reads::midi_in_played(&a.graph)),
        R::LaneInput(a) => write(out, &reads::lane_input(&a.graph, &a.track)),
        R::RackFor(a) => write(out, &reads::rack_for(&a.graph, &a.track)),
        R::RackKeyNames(a) => write(out, &reads::rack_key_names(&a.rack)),
        R::WithoutNotes(a) => {
            let notes: HashSet<String> = a.notes.into_iter().collect();
            write(out, &reads::without_notes(a.graph, &notes))
        }
        R::TemplateGraph(a) => write(out, &template_graph(&a)),
        R::GivesChannel(a) => write(out, &reads::gives_channel(&a.line)),
        R::HasAudibleInput(a) => write(out, &reads::has_audible_input(&a.graph)),
        R::RackHits(a) => write(out, &reads::rack_hits(&a.tracks, &a.graph)),
        R::ClipNameParts(a) => write(out, &reads::clip_name_parts(&a.ref_)),
        R::FixedMaster(a) => write(out, &reads::fixed_master(a.master_db, a.peak_db.unwrap_or(f64::NEG_INFINITY), MASTER_MIN_DB, MASTER_MAX_DB)),
        R::ShellView(a) => write(out, &reads::shell_view(a.tempo, a.num, a.den, a.count_in, &a.tracks)),
        R::LaneMix(a) => {
            let (lanes, pictures) = reads::lane_mix(&a.tracks, &a.groups, &a.video);
            write(out, &json!({ "lanes": lanes, "pictures": pictures }))
        }
        R::Blanks => write(out, &reads::blanks()),
        R::EnginePlugin(a) => {
            let plugin = song::modules::Plugin { plugin_type: a.kind, name: a.name, ..Default::default() };
            let (kind, name) = song::console::engine_plugin(Some(&plugin), &a.fallback, a.emits.as_deref());
            write(out, &json!({ "type": kind, "name": name }))
        }
        R::TrimFor(a) => write(out, &reads::trim_for(a.peak_db.unwrap_or(f64::NEG_INFINITY))),
        R::TapTempo(a) => write(out, &reads::tap_tempo(&a.taps, a.now)),
        R::FolderName(a) => write(out, &reads::folder_name(&a.raw, &a.suffixes)),
        R::FollowRename(a) => write(out, &reads::follow_rename(a.open.as_deref(), &a.before, &a.after)),
        R::ShapeOfSurfaces(a) => write(out, &reads::shape_of_surfaces(a.surfaces.as_deref())),
    }
}
