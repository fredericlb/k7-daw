//! fake_state.rs — the state area of the fake engine: the table over a fake host, the sinks it was given, and
//! helpers that fire them from the message thread as the real timer does, with a frame the test spelled out.
//! What a reader gets wrong: nothing ticks by itself; a test calls `tick`, which posts the frame to the thread
//! that runs the host, so a sink sees the same thread and the same borrowed arrays as in the real engine.

use super::Engine;
use super::fake::{log, with_host_state, with_state};
use super::ffi::{K7Host, K7Str};
use super::state::{
    K7Envelope, K7LoadEntry, K7ParamInfo, K7ParamInfoEach, K7ParamState, K7ParamValue,
    K7PlatterEntry, K7RecorderEntry, K7StateApi, K7StateFrame, K7StateSinks, K7TakeInfo,
    K7WaveformEntry, ParamInfo, Recorded,
};
use std::collections::HashMap;
use std::ffi::c_void;

/// What the fake keeps of the area, inside `FakeState`.
#[derive(Default)]
pub struct Area {
    sinks: Option<SendSinks>,
    infos: HashMap<(String, String), Vec<ParamInfo>>,
    pub interval_ms: i32,
}

#[derive(Clone, Copy)]
struct SendSinks(K7StateSinks);

impl SendSinks {
    // A method, so a closure captures the whole wrapper and not its non-Send field.
    fn get(&self) -> K7StateSinks {
        self.0
    }
}

// The sinks' context is the server's leaked box, whose closures are Send and Sync.
unsafe impl Send for SendSinks {}

pub static STATE: K7StateApi = K7StateApi {
    start,
    rewind_recorder_envelopes,
    watch_plugin_params,
    plugin_param_infos,
};

unsafe extern "C" fn start(host: *mut K7Host, sinks: *const K7StateSinks, interval_ms: i32) {
    let sinks = unsafe { *sinks };
    with_host_state(host, |s| {
        s.state_area.sinks = Some(SendSinks(sinks));
        s.state_area.interval_ms = interval_ms;
    });
    log(host, &format!("state_start {interval_ms}"));
}

unsafe extern "C" fn rewind_recorder_envelopes(host: *mut K7Host, session: K7Str) {
    log(
        host,
        &format!("rewind_recorder_envelopes {}", unsafe {
            session.to_string_lossy()
        }),
    );
}

unsafe extern "C" fn watch_plugin_params(host: *mut K7Host, session: K7Str, id: K7Str, on: i32) {
    let (session, id) = unsafe { (session.to_string_lossy(), id.to_string_lossy()) };
    log(host, &format!("watch_plugin_params {session} {id} {on}"));
}

unsafe extern "C" fn plugin_param_infos(
    host: *mut K7Host,
    session: K7Str,
    id: K7Str,
    ctx: *mut c_void,
    each: K7ParamInfoEach,
) {
    let key = unsafe { (session.to_string_lossy(), id.to_string_lossy()) };
    let infos = with_host_state(host, |s| s.state_area.infos.get(&key).cloned())
        .flatten()
        .unwrap_or_default();
    let Some(each) = each else { return };
    for p in infos {
        let states: Vec<K7ParamState> = p
            .states
            .iter()
            .map(|(label, value)| K7ParamState {
                label: K7Str::of(label),
                value: *value,
            })
            .collect();
        let raw = K7ParamInfo {
            name: K7Str::of(&p.name),
            value: p.value,
            min: p.min,
            max: p.max,
            has_default: i32::from(p.default.is_some()),
            def: p.default.unwrap_or(0.0),
            states: states.as_ptr(),
            state_count: states.len(),
        };
        unsafe { each(ctx, &raw) };
    }
}

/// One recorder node of a frame a test spells out.
#[derive(Clone, Default)]
pub struct RecorderSpec {
    pub id: String,
    pub recording: bool,
    pub seconds: f64,
    pub takes: Vec<(String, f64, bool)>,
    pub envelope: Vec<(f32, f32)>,
    pub envelope_from: u64,
}

/// The cost report of a frame: `(plugin, cpu, bytes)` for each, and the process's bytes.
pub type LoadSpec = (Vec<(String, f64, i64)>, i64);

/// A state frame a test spells out; `Default` is not silent, so start from `FrameSpec::idle`.
#[derive(Clone, Default)]
pub struct FrameSpec {
    pub session: String,
    pub playing: bool,
    pub playhead: f64,
    pub loop_on: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub levels: Vec<f32>,
    pub input_levels: Vec<f32>,
    pub armed: Vec<String>,
    pub record_enabled: bool,
    pub master_db: f32,
    pub master_clipped: bool,
    pub waveforms: Vec<(String, Vec<(f32, f32)>)>,
    pub platters: Vec<(String, f64)>,
    pub recorders: Vec<RecorderSpec>,
    /// `(plugin, cpu, bytes)` and the process's bytes, on a frame that carries the cost report.
    pub load: Option<LoadSpec>,
}

impl FrameSpec {
    /// Nothing playing, the master silent.
    pub fn idle(session: &str) -> Self {
        FrameSpec {
            session: session.to_string(),
            master_db: -100.0,
            ..FrameSpec::default()
        }
    }

    /// The raw frame over this spec's own arrays, for the call of `f`.
    pub fn with_raw<R>(&self, f: impl FnOnce(&K7StateFrame) -> R) -> R {
        let envs = |v: &[(f32, f32)]| -> Vec<K7Envelope> {
            v.iter()
                .map(|&(min, max)| K7Envelope { min, max })
                .collect()
        };
        let armed: Vec<K7Str> = self.armed.iter().map(|s| K7Str::of(s)).collect();
        let points: Vec<Vec<K7Envelope>> = self.waveforms.iter().map(|(_, e)| envs(e)).collect();
        let waveforms: Vec<K7WaveformEntry> = self
            .waveforms
            .iter()
            .zip(&points)
            .map(|((key, _), p)| K7WaveformEntry {
                key: K7Str::of(key),
                points: p.as_ptr(),
                count: p.len(),
            })
            .collect();
        let platters: Vec<K7PlatterEntry> = self
            .platters
            .iter()
            .map(|(id, seconds)| K7PlatterEntry {
                id: K7Str::of(id),
                seconds: *seconds,
            })
            .collect();
        let takes: Vec<Vec<K7TakeInfo>> = self
            .recorders
            .iter()
            .map(|r| {
                r.takes
                    .iter()
                    .map(|(file, seconds, damaged)| K7TakeInfo {
                        file: K7Str::of(file),
                        seconds: *seconds,
                        damaged: i32::from(*damaged),
                    })
                    .collect()
            })
            .collect();
        let pictures: Vec<Vec<K7Envelope>> =
            self.recorders.iter().map(|r| envs(&r.envelope)).collect();
        let recorders: Vec<K7RecorderEntry> = self
            .recorders
            .iter()
            .zip(takes.iter().zip(&pictures))
            .map(|(r, (t, p))| K7RecorderEntry {
                id: K7Str::of(&r.id),
                recording: i32::from(r.recording),
                seconds: r.seconds,
                takes: t.as_ptr(),
                take_count: t.len(),
                envelope: p.as_ptr(),
                envelope_count: p.len(),
                envelope_from: r.envelope_from,
            })
            .collect();
        let (loads, process_bytes) = match &self.load {
            Some((plugins, bytes)) => (
                plugins
                    .iter()
                    .map(|(id, cpu, bytes)| K7LoadEntry {
                        id: K7Str::of(id),
                        cpu: *cpu,
                        bytes: *bytes,
                    })
                    .collect(),
                *bytes,
            ),
            None => (Vec::new(), -1),
        };
        let raw = K7StateFrame {
            session: K7Str::of(&self.session),
            playing: i32::from(self.playing),
            playhead: self.playhead,
            loop_on: i32::from(self.loop_on),
            loop_start: self.loop_start,
            loop_end: self.loop_end,
            levels: self.levels.as_ptr(),
            level_count: self.levels.len(),
            input_levels: self.input_levels.as_ptr(),
            input_level_count: self.input_levels.len(),
            armed: armed.as_ptr(),
            armed_count: armed.len(),
            record_enabled: i32::from(self.record_enabled),
            master_db: self.master_db,
            master_clipped: i32::from(self.master_clipped),
            waveforms: waveforms.as_ptr(),
            waveform_count: waveforms.len(),
            platters: platters.as_ptr(),
            platter_count: platters.len(),
            recorders: recorders.as_ptr(),
            recorder_count: recorders.len(),
            has_load: i32::from(self.load.is_some()),
            loads: loads.as_ptr(),
            load_count: loads.len(),
            process_bytes,
        };
        f(&raw)
    }
}

fn sinks_of(engine: &Engine) -> Option<SendSinks> {
    with_state(engine, |s| s.state_area.sinks).flatten()
}

/// Whether the server started the state timer.
pub fn started(engine: &Engine) -> bool {
    sinks_of(engine).is_some()
}

/// One tick of the state timer for `spec`, on the thread that runs the host; false when no timer started.
pub fn tick(engine: &Engine, spec: FrameSpec) -> bool {
    let Some(sinks) = sinks_of(engine) else {
        return false;
    };
    engine.post(move |_| {
        let sinks = sinks.get();
        if let Some(state) = sinks.state {
            spec.with_raw(|raw| unsafe { state(sinks.ctx, raw) });
        }
    })
}

/// A take finished, as the engine's recorder reports it.
pub fn recorded(engine: &Engine, take: Recorded) -> bool {
    let Some(sinks) = sinks_of(engine) else {
        return false;
    };
    engine.post(move |_| {
        let sinks = sinks.get();
        if let Some(sink) = sinks.recorded {
            unsafe {
                sink(
                    sinks.ctx,
                    K7Str::of(&take.session),
                    K7Str::of(&take.track),
                    K7Str::of(&take.file),
                    take.start,
                    take.length,
                )
            };
        }
    })
}

/// A plugin editor closed: its parameters as the JSON text the engine built.
pub fn plugin_params(engine: &Engine, session: &str, id: &str, json: &str) -> bool {
    let Some(sinks) = sinks_of(engine) else {
        return false;
    };
    let (session, id, json) = (session.to_string(), id.to_string(), json.to_string());
    engine.post(move |_| {
        let sinks = sinks.get();
        if let Some(sink) = sinks.params {
            unsafe {
                sink(
                    sinks.ctx,
                    K7Str::of(&session),
                    K7Str::of(&id),
                    K7Str::of(&json),
                )
            };
        }
    })
}

/// A watched value moved: every parameter of the plugin.
pub fn plugin_params_live(
    engine: &Engine,
    session: &str,
    id: &str,
    params: Vec<(String, f64)>,
) -> bool {
    let Some(sinks) = sinks_of(engine) else {
        return false;
    };
    let (session, id) = (session.to_string(), id.to_string());
    engine.post(move |_| {
        let sinks = sinks.get();
        if let Some(sink) = sinks.params_live {
            let raw: Vec<K7ParamValue> = params
                .iter()
                .map(|(name, value)| K7ParamValue {
                    name: K7Str::of(name),
                    value: *value,
                })
                .collect();
            unsafe {
                sink(
                    sinks.ctx,
                    K7Str::of(&session),
                    K7Str::of(&id),
                    raw.as_ptr(),
                    raw.len(),
                )
            };
        }
    })
}

/// What `plugin_param_infos` answers for a plugin of a session.
pub fn set_param_infos(engine: &Engine, session: &str, id: &str, infos: Vec<ParamInfo>) {
    with_state(engine, |s| {
        s.state_area
            .infos
            .insert((session.to_string(), id.to_string()), infos)
    });
}
