//! state_frame.rs — the ~20 Hz `state` frame as serde structs, the wire's tree field for field and its rounding
//! (a peak to four decimals, a level to a hundredth of a dB; the playhead, needles and master keep full precision).
//! What a reader gets wrong: `master` is a float in the engine, widened to a double and printed whole, as
//! nlohmann did, so a frame holds a 17-digit master level beside its rounded `levels`.

use crate::engine::state::{K7Envelope, StateFrame};
use crate::wire::frames::tagged;
#[cfg(feature = "ts")]
use crate::wire::rec::Rec;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

fn peak(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

fn level(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

/// One min/max envelope point, covering a bucket of N samples (256 at 44.1 kHz ≈ 5.8 ms), normalised -1..1.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WaveformEnvelope {
    pub min: f64,
    pub max: f64,
}

/// One take a recorder node finished: where the engine wrote it, and what it cost.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RecorderTake {
    /// ABSOLUTE, on the engine's machine — the file service is what reads it, and it runs there.
    pub file: String,
    pub seconds: f64,
    /// The ring lapped: frames are MISSING from this file. Reported rather than left to be noticed.
    pub damaged: bool,
}

/// One recorder node as the state frame reports it — free-running, so none of this is transport.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RecorderState {
    pub recording: bool,
    /// The take being written; 0 when idle.
    pub seconds: f64,
    /// Oldest first, so the last one is what `clear` pops.
    pub takes: Vec<RecorderTake>,
    /// Points made SINCE THE LAST FRAME. The client APPENDS them — see engine docs/recorder.md.
    pub envelope: Vec<WaveformEnvelope>,
    /// Where those points START in the take: the count the client already holds, or 0 for a socket just connected.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub from: u64,
}

/// One plugin's cost.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginLoad {
    /// Its share of an audio block's time, 0..1 — audio-thread work only, not its own threads'.
    pub cpu: f64,
    /// The bytes it says it holds, from our own plugins only; absent for one that cannot say.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub bytes: Option<i64>,
}

/// Every plugin's cost and the engine's own memory, as one state frame reports them.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EngineLoad {
    #[cfg_attr(feature = "ts", ts(as = "Rec<PluginLoad>"))]
    pub plugins: BTreeMap<String, PluginLoad>,
    /// The engine process's memory in bytes, as its OS counts it; absent where the platform will not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub process: Option<i64>,
}

/// The transport loop the engine wraps in, in seconds.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoopRegion {
    pub on: bool,
    pub start: f64,
    pub end: f64,
}

/// A live engine state frame — the ~20 Hz meter/transport push.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "EngineState"))]
#[serde(rename_all = "camelCase")]
pub struct EngineStateFrame {
    /// WHICH project this frame describes; absent from an engine older than sessions — see docs/sessions.md.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub session: Option<String>,
    pub playing: bool,
    /// Seconds.
    pub playhead: f64,
    /// Absent from an engine older than it; a reloaded page draws it from here.
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "loop", optional))]
    pub loop_region: Option<LoopRegion>,
    /// Per-track OUTPUT dB (post-fader), same index order as the tracks.
    pub levels: Vec<f64>,
    /// Per-track live INPUT level in dB, sent for every track so a client can index it like `levels`; -100 with no input assigned.
    pub input_levels: Vec<f64>,
    /// The record-armed track ids: session state the engine owns, so it rides the state frame rather than the doc.
    pub armed: Vec<String>,
    /// This session's record enable: its armed tracks record only while this is on AND rolling.
    pub record_enabled: bool,
    /// The MASTER output in dB (-100 == silent) — one number for the whole song, read off a level-meter plugin on the master list.
    pub master: f64,
    /// Whether the master went over full scale since the last frame, from the meter's LATCHED peak rather than this 20 Hz sample.
    pub master_clipped: bool,
    /// Per-module waveform envelopes keyed "chain:module:side", sampled at the state-frame rate; empty with no taps.
    #[cfg_attr(feature = "ts", ts(as = "Rec<Vec<WaveformEnvelope>>"))]
    pub waveform: BTreeMap<String, Vec<WaveformEnvelope>>,
    /// Each PLATTER's needle in seconds of its OWN record, by plugin id — `playhead` is not its clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Rec<f64>>"))]
    pub platters: Option<BTreeMap<String, f64>>,
    /// Each RECORDER node, by plugin id.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Rec<RecorderState>>"))]
    pub recorders: Option<BTreeMap<String, RecorderState>>,
    /// What things COST, on one frame in four — absent on the others, which keep the last. engine docs/load.md.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub load: Option<EngineLoad>,
}

fn levels(db: &[f32]) -> Vec<f64> {
    db.iter().map(|&v| level(v)).collect()
}

fn envelope(points: &[K7Envelope]) -> Vec<WaveformEnvelope> {
    points
        .iter()
        .map(|e| WaveformEnvelope {
            min: peak(e.min),
            max: peak(e.max),
        })
        .collect()
}

fn load_of(s: &StateFrame<'_>) -> Option<EngineLoad> {
    let load = s.load()?;
    Some(EngineLoad {
        plugins: load
            .plugins
            .into_iter()
            .map(|(id, cpu, bytes)| {
                (
                    id,
                    PluginLoad {
                        cpu,
                        bytes: (bytes >= 0).then_some(bytes),
                    },
                )
            })
            .collect(),
        process: (load.process_bytes >= 0).then_some(load.process_bytes),
    })
}

/// The frame of one session, ready to print.
pub fn frame(s: &StateFrame<'_>) -> Value {
    let recorders = s
        .recorders()
        .into_iter()
        .map(|r| {
            let takes = r
                .takes
                .iter()
                .map(|(file, seconds, damaged)| RecorderTake {
                    file: file.clone(),
                    seconds: *seconds,
                    damaged: *damaged,
                })
                .collect();
            let state = RecorderState {
                recording: r.recording,
                seconds: r.seconds,
                takes,
                envelope: envelope(r.envelope),
                from: r.envelope_from,
            };
            (r.id, state)
        })
        .collect();
    tagged(
        "state",
        &EngineStateFrame {
            session: Some(s.session()),
            playing: s.playing(),
            playhead: s.playhead(),
            loop_region: Some(LoopRegion {
                on: s.loop_on(),
                start: s.loop_start(),
                end: s.loop_end(),
            }),
            levels: levels(s.levels()),
            input_levels: levels(s.input_levels()),
            armed: s.armed(),
            record_enabled: s.record_enabled(),
            master: f64::from(s.master_db()),
            master_clipped: s.master_clipped(),
            waveform: s
                .waveforms()
                .into_iter()
                .map(|(k, p)| (k, envelope(p)))
                .collect(),
            platters: Some(s.platters().into_iter().collect()),
            recorders: Some(recorders),
            load: load_of(s),
        },
    )
}
