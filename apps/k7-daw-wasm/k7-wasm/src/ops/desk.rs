// ops/desk.rs — the desk.* operations: the DJ desk's rules over the `turntables` crate, from the deck document to the set list.
// A reader would get wrong that a null in `tempo` or `compiled` means absent: the page sends JSON, and an undefined key is simply missing.

use crate::error::WasmError;
use crate::ops::write;
use chains::bindings::BindGraph;
use serde::Deserialize;
use song::doc::ProjectDoc;
#[cfg(feature = "ts")]
use std::collections::HashMap;
use turntables::beats::{GridOf, Needles, align_seek, beat_phase};
use turntables::bindings::{
    DeskBinding, Mapping, offline_publish, resolve_bindings, stretch_targets, unmapped, value_for,
};
use turntables::deck::{
    crossfade_levels, nudged_tempos, platter_key, platter_rate, platters_of, synced_tempos, with_keylock,
};
use turntables::deck_doc::{DeckPlaying, DeckSet, document};
use turntables::grid::corrected;
use turntables::loops::{
    DeckLoop, LoopEdges, beat_jump, beat_seconds, beats_between, can_loop, loop_edges, loop_params, loop_span, resized,
    snap_to_beat,
};
use turntables::metronome::metronome_deck;
use turntables::rebuild::worth_a_rebuild;
use turntables::set_list::resolve_set;
use turntables::vocab::{DeckLoad, DeckLoads, DeckTempo, DeckTempos};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PhaseArgs {
    pub transients: Vec<f64>,
    pub bpm: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AlignArgs {
    pub master: GridOf,
    pub slave: GridOf,
    pub at: Needles,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct CorrectedArgs {
    pub tempo: DeckTempo,
    #[serde(default)]
    pub bpm: Option<f64>,
}

/// A tempo the deck may not have.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct TempoArgs {
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SnapArgs {
    pub at: f64,
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResizedArgs {
    pub beats: f64,
    pub factor: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct BetweenArgs {
    pub from: f64,
    pub to: f64,
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct EdgesArgs {
    #[serde(rename = "loop")]
    #[cfg_attr(feature = "ts", ts(rename = "loop"))]
    pub lp: DeckLoop,
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct JumpArgs {
    pub needle: f64,
    pub beats: f64,
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
    pub quantise: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ParamsArgs {
    #[serde(default)]
    pub edges: Option<LoopEdges>,
    pub roll: bool,
    #[serde(default)]
    pub resize: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SpanArgs {
    #[serde(default, rename = "loop")]
    #[cfg_attr(feature = "ts", ts(rename = "loop"))]
    pub lp: Option<DeckLoop>,
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ClickArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, Option<DeckLoad>>"))]
    pub loads: DeckLoads,
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub tempos: DeckTempos,
    #[serde(default)]
    pub master: Option<String>,
}

/// The document to send: the compiled project with the decks filled in, or the stand-in when there is none.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct DocumentArgs {
    #[serde(default)]
    pub compiled: Option<ProjectDoc>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub set: DeckSet,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct PlattersArgs {
    pub deck: String,
    #[serde(default)]
    pub load: Option<DeckLoad>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RateArgs {
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
    pub playing: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct KeyArgs {
    #[serde(default)]
    pub tempo: Option<DeckTempo>,
    pub holding: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NudgeArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub faders: DeckTempos,
    pub deck: String,
    pub bpm: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KeylockArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub tempos: DeckTempos,
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, bool>"))]
    pub on: DeckPlaying,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SyncArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub tempos: DeckTempos,
    #[serde(default)]
    pub master: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MixArgs {
    pub mix: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct MappingArgs {
    #[serde(default)]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<HashMap<String, Option<chains::bindings::SurfaceBinding>>>")
    )]
    pub mapping: Option<Mapping>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ResolveArgs {
    #[serde(default)]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<HashMap<String, Option<chains::bindings::SurfaceBinding>>>")
    )]
    pub mapping: Option<Mapping>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph | null"))]
    pub graph: Option<BindGraph>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StretchArgs {
    pub bindings: Vec<DeskBinding>,
    #[serde(rename = "byDeck")]
    #[cfg_attr(feature = "ts", ts(rename = "byDeck"))]
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub by_deck: DeckTempos,
    pub plugins: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PublishArgs {
    pub bindings: Vec<DeskBinding>,
    #[serde(rename = "byDeck")]
    #[cfg_attr(feature = "ts", ts(rename = "byDeck"))]
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub by_deck: DeckTempos,
    #[serde(rename = "sourceParam")]
    #[cfg_attr(feature = "ts", ts(rename = "sourceParam"))]
    pub source_param: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValueForArgs {
    pub binding: DeskBinding,
    pub value: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SetArgs {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub files: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RebuildArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub sent: DeckTempos,
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub now: DeckTempos,
}

/// The desk family's requests: `{"op": "desk.beat_phase", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum DeskRequest {
    #[serde(rename = "desk.beat_phase")]
    BeatPhase(PhaseArgs),
    #[serde(rename = "desk.align_seek")]
    AlignSeek(AlignArgs),
    #[serde(rename = "desk.corrected")]
    Corrected(CorrectedArgs),
    #[serde(rename = "desk.beat_seconds")]
    BeatSeconds(TempoArgs),
    #[serde(rename = "desk.can_loop")]
    CanLoop(TempoArgs),
    #[serde(rename = "desk.snap_to_beat")]
    SnapToBeat(SnapArgs),
    #[serde(rename = "desk.resized")]
    Resized(ResizedArgs),
    #[serde(rename = "desk.beats_between")]
    BeatsBetween(BetweenArgs),
    #[serde(rename = "desk.loop_edges")]
    LoopEdges(EdgesArgs),
    #[serde(rename = "desk.beat_jump")]
    BeatJump(JumpArgs),
    #[serde(rename = "desk.loop_params")]
    LoopParams(ParamsArgs),
    #[serde(rename = "desk.loop_span")]
    LoopSpan(SpanArgs),
    #[serde(rename = "desk.metronome_deck")]
    MetronomeDeck(ClickArgs),
    #[serde(rename = "desk.document")]
    Document(Box<DocumentArgs>),
    #[serde(rename = "desk.platters_of")]
    PlattersOf(PlattersArgs),
    #[serde(rename = "desk.platter_rate")]
    PlatterRate(RateArgs),
    #[serde(rename = "desk.platter_key")]
    PlatterKey(KeyArgs),
    #[serde(rename = "desk.nudged_tempos")]
    NudgedTempos(NudgeArgs),
    #[serde(rename = "desk.with_keylock")]
    WithKeylock(KeylockArgs),
    #[serde(rename = "desk.synced_tempos")]
    SyncedTempos(SyncArgs),
    #[serde(rename = "desk.crossfade_levels")]
    CrossfadeLevels(MixArgs),
    #[serde(rename = "desk.unmapped_controls")]
    UnmappedControls(MappingArgs),
    #[serde(rename = "desk.resolve_bindings")]
    ResolveBindings(ResolveArgs),
    #[serde(rename = "desk.stretch_targets")]
    StretchTargets(StretchArgs),
    #[serde(rename = "desk.offline_publish")]
    OfflinePublish(PublishArgs),
    #[serde(rename = "desk.value_for")]
    ValueFor(Box<ValueForArgs>),
    #[serde(rename = "desk.resolve_set")]
    ResolveSet(SetArgs),
    #[serde(rename = "desk.worth_a_rebuild")]
    WorthARebuild(RebuildArgs),
}

pub fn dispatch(request: DeskRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use DeskRequest as R;
    match request {
        R::BeatPhase(a) => write(out, &beat_phase(&a.transients, a.bpm)),
        R::AlignSeek(a) => write(out, &align_seek(a.master, a.slave, a.at)),
        R::Corrected(a) => write(out, &corrected(&a.tempo, a.bpm)),
        R::BeatSeconds(a) => write(out, &beat_seconds(a.tempo.as_ref())),
        R::CanLoop(a) => write(out, &can_loop(a.tempo.as_ref())),
        R::SnapToBeat(a) => write(out, &snap_to_beat(a.at, a.tempo.as_ref())),
        R::Resized(a) => write(out, &resized(a.beats, a.factor)),
        R::BeatsBetween(a) => write(out, &beats_between(a.from, a.to, a.tempo.as_ref())),
        R::LoopEdges(a) => write(out, &loop_edges(&a.lp, a.tempo.as_ref())),
        R::BeatJump(a) => write(out, &beat_jump(a.needle, a.beats, a.tempo.as_ref(), a.quantise)),
        R::LoopParams(a) => write(out, &loop_params(a.edges, a.roll, a.resize)),
        R::LoopSpan(a) => write(out, &loop_span(a.lp.as_ref(), a.tempo.as_ref())),
        R::MetronomeDeck(a) => write(out, &metronome_deck(&a.loads, &a.tempos, a.master.as_deref())),
        R::Document(a) => write(out, &document(a.compiled, &a.set)),
        R::PlattersOf(a) => write(out, &platters_of(&a.deck, a.load.as_ref())),
        R::PlatterRate(a) => write(out, &platter_rate(a.tempo.as_ref(), a.playing)),
        R::PlatterKey(a) => write(out, &platter_key(a.tempo.as_ref(), a.holding)),
        R::NudgedTempos(a) => write(out, &nudged_tempos(&a.faders, &a.deck, a.bpm)),
        R::WithKeylock(a) => write(out, &with_keylock(&a.tempos, &a.on)),
        R::SyncedTempos(a) => write(out, &synced_tempos(&a.tempos, a.master.as_deref())),
        R::CrossfadeLevels(a) => write(out, &crossfade_levels(a.mix)),
        R::UnmappedControls(a) => write(out, &unmapped(&a.mapping.unwrap_or_default())),
        R::ResolveBindings(a) => write(
            out,
            &resolve_bindings(&a.mapping.unwrap_or_default(), &a.graph.unwrap_or_default()),
        ),
        R::StretchTargets(a) => write(out, &stretch_targets(&a.bindings, &a.by_deck, &a.plugins)),
        R::OfflinePublish(a) => write(out, &offline_publish(&a.bindings, &a.by_deck, &a.source_param)),
        R::ValueFor(a) => write(out, &value_for(&a.binding, a.value)),
        R::ResolveSet(a) => write(out, &resolve_set(a.text.as_deref(), a.files.as_deref())),
        R::WorthARebuild(a) => write(out, &worth_a_rebuild(&a.sent, &a.now)),
    }
}
