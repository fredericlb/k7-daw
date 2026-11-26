//! state.rs — the state area of the C ABI (`k7_engine_state.h`) made safe: the engine's state timer and its
//! sinks (a frame per session, a take, a plugin's parameters), and the calls on a watched plugin.
//! What a reader gets wrong: a `StateFrame` borrows the engine's arrays for the one call of the sink, on the
//! message thread, so a sink copies what it keeps and never blocks; an engine without the area ignores the calls.

use super::ffi::{K7Host, K7Str};
use super::{Api, Mt};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct K7Envelope {
    pub min: f32,
    pub max: f32,
}

#[repr(C)]
pub struct K7TakeInfo {
    pub file: K7Str,
    pub seconds: f64,
    pub damaged: i32,
}

#[repr(C)]
pub struct K7WaveformEntry {
    pub key: K7Str,
    pub points: *const K7Envelope,
    pub count: usize,
}

#[repr(C)]
pub struct K7PlatterEntry {
    pub id: K7Str,
    pub seconds: f64,
}

#[repr(C)]
pub struct K7RecorderEntry {
    pub id: K7Str,
    pub recording: i32,
    pub seconds: f64,
    pub takes: *const K7TakeInfo,
    pub take_count: usize,
    pub envelope: *const K7Envelope,
    pub envelope_count: usize,
    pub envelope_from: u64,
}

#[repr(C)]
pub struct K7LoadEntry {
    pub id: K7Str,
    pub cpu: f64,
    pub bytes: i64,
}

#[repr(C)]
pub struct K7StateFrame {
    pub session: K7Str,
    pub playing: i32,
    pub playhead: f64,
    pub loop_on: i32,
    pub loop_start: f64,
    pub loop_end: f64,
    pub levels: *const f32,
    pub level_count: usize,
    pub input_levels: *const f32,
    pub input_level_count: usize,
    pub armed: *const K7Str,
    pub armed_count: usize,
    pub record_enabled: i32,
    pub master_db: f32,
    pub master_clipped: i32,
    pub waveforms: *const K7WaveformEntry,
    pub waveform_count: usize,
    pub platters: *const K7PlatterEntry,
    pub platter_count: usize,
    pub recorders: *const K7RecorderEntry,
    pub recorder_count: usize,
    pub has_load: i32,
    pub loads: *const K7LoadEntry,
    pub load_count: usize,
    pub process_bytes: i64,
}

#[repr(C)]
pub struct K7ParamValue {
    pub name: K7Str,
    pub value: f64,
}

#[repr(C)]
pub struct K7ParamState {
    pub label: K7Str,
    pub value: f64,
}

#[repr(C)]
pub struct K7ParamInfo {
    pub name: K7Str,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub has_default: i32,
    pub def: f64,
    pub states: *const K7ParamState,
    pub state_count: usize,
}

pub type K7StateSink = Option<unsafe extern "C" fn(ctx: *mut c_void, frame: *const K7StateFrame)>;
pub type K7RecordedSink = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        session: K7Str,
        track: K7Str,
        file: K7Str,
        start: f64,
        length: f64,
    ),
>;
pub type K7ParamsSink =
    Option<unsafe extern "C" fn(ctx: *mut c_void, session: K7Str, id: K7Str, params_json: K7Str)>;
pub type K7ParamsLiveSink = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        session: K7Str,
        id: K7Str,
        params: *const K7ParamValue,
        count: usize,
    ),
>;
pub type K7ParamInfoEach = Option<unsafe extern "C" fn(ctx: *mut c_void, info: *const K7ParamInfo)>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct K7StateSinks {
    pub ctx: *mut c_void,
    pub state: K7StateSink,
    pub recorded: K7RecordedSink,
    pub params: K7ParamsSink,
    pub params_live: K7ParamsLiveSink,
}

/// `k7_engine_state.h`'s table, field for field (`tests/all/engine_abi.rs` holds the names to the header).
#[repr(C)]
pub struct K7StateApi {
    pub start:
        unsafe extern "C" fn(host: *mut K7Host, sinks: *const K7StateSinks, interval_ms: i32),
    pub rewind_recorder_envelopes: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
    pub watch_plugin_params:
        unsafe extern "C" fn(host: *mut K7Host, session: K7Str, id: K7Str, on: i32),
    pub plugin_param_infos: unsafe extern "C" fn(
        host: *mut K7Host,
        session: K7Str,
        id: K7Str,
        ctx: *mut c_void,
        each: K7ParamInfoEach,
    ),
}

impl K7StateApi {
    pub const FIELDS: &'static [&'static str] = &[
        "start",
        "rewind_recorder_envelopes",
        "watch_plugin_params",
        "plugin_param_infos",
    ];
}

unsafe impl Sync for K7StateApi {}

/// An array the engine lent, empty when it lent none.
unsafe fn lent<'a, T>(ptr: *const T, n: usize) -> &'a [T] {
    if ptr.is_null() || n == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, n) }
    }
}

fn text(s: K7Str) -> String {
    unsafe { s.to_string_lossy() }
}

pub struct RecorderView<'a> {
    pub id: String,
    pub recording: bool,
    pub seconds: f64,
    /// `(file, seconds, damaged)`.
    pub takes: Vec<(String, f64, bool)>,
    pub envelope: &'a [K7Envelope],
    pub envelope_from: u64,
}

/// What the cost report of a frame carries; a frame without one says so with `None`.
pub struct LoadView {
    /// `(plugin id, cpu, bytes)`, bytes -1 when the plugin cannot say.
    pub plugins: Vec<(String, f64, i64)>,
    pub process_bytes: i64,
}

/// One session's snapshot, borrowed from the engine for the call of the sink.
pub struct StateFrame<'a>(&'a K7StateFrame);

impl<'a> StateFrame<'a> {
    /// A view of a frame the engine (or the fake) holds: valid while `raw` and what it points to are.
    pub fn new(raw: &'a K7StateFrame) -> Self {
        StateFrame(raw)
    }

    pub fn session(&self) -> String {
        text(self.0.session)
    }
    pub fn playing(&self) -> bool {
        self.0.playing != 0
    }
    pub fn playhead(&self) -> f64 {
        self.0.playhead
    }
    pub fn loop_on(&self) -> bool {
        self.0.loop_on != 0
    }
    pub fn loop_start(&self) -> f64 {
        self.0.loop_start
    }
    pub fn loop_end(&self) -> f64 {
        self.0.loop_end
    }
    pub fn levels(&self) -> &'a [f32] {
        unsafe { lent(self.0.levels, self.0.level_count) }
    }
    pub fn input_levels(&self) -> &'a [f32] {
        unsafe { lent(self.0.input_levels, self.0.input_level_count) }
    }
    pub fn armed(&self) -> Vec<String> {
        unsafe { lent(self.0.armed, self.0.armed_count) }
            .iter()
            .map(|s| text(*s))
            .collect()
    }
    pub fn record_enabled(&self) -> bool {
        self.0.record_enabled != 0
    }
    pub fn master_db(&self) -> f32 {
        self.0.master_db
    }
    pub fn master_clipped(&self) -> bool {
        self.0.master_clipped != 0
    }
    pub fn waveforms(&self) -> Vec<(String, &'a [K7Envelope])> {
        unsafe { lent(self.0.waveforms, self.0.waveform_count) }
            .iter()
            .map(|w| (text(w.key), unsafe { lent(w.points, w.count) }))
            .collect()
    }
    pub fn platters(&self) -> Vec<(String, f64)> {
        unsafe { lent(self.0.platters, self.0.platter_count) }
            .iter()
            .map(|p| (text(p.id), p.seconds))
            .collect()
    }
    pub fn recorders(&self) -> Vec<RecorderView<'a>> {
        unsafe { lent(self.0.recorders, self.0.recorder_count) }
            .iter()
            .map(|r| RecorderView {
                id: text(r.id),
                recording: r.recording != 0,
                seconds: r.seconds,
                takes: unsafe { lent(r.takes, r.take_count) }
                    .iter()
                    .map(|t| (text(t.file), t.seconds, t.damaged != 0))
                    .collect(),
                envelope: unsafe { lent(r.envelope, r.envelope_count) },
                envelope_from: r.envelope_from,
            })
            .collect()
    }
    pub fn load(&self) -> Option<LoadView> {
        (self.0.has_load != 0).then(|| LoadView {
            plugins: unsafe { lent(self.0.loads, self.0.load_count) }
                .iter()
                .map(|l| (text(l.id), l.cpu, l.bytes))
                .collect(),
            process_bytes: self.0.process_bytes,
        })
    }
}

/// A finished take: the file is closed and named; `start` and `length` are in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    pub session: String,
    pub track: String,
    pub file: String,
    pub start: f64,
    pub length: f64,
}

/// What a plugin parameter is and how a control draws it, in the parameter's own units.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub name: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub default: Option<f64>,
    /// `(label, value)` of a discrete parameter's settings; empty when continuous.
    pub states: Vec<(String, f64)>,
}

/// `(session, plugin id, the params' JSON text)`.
pub type ParamsSink = Box<dyn Fn(&str, &str, &str) + Send + Sync>;
/// `(session, plugin id, every parameter)`.
pub type ParamsLiveSink = Box<dyn Fn(&str, &str, &[(String, f64)]) + Send + Sync>;

/// What the engine's timer and events call, on the message thread; set once per run and never freed.
pub struct StateSinks {
    pub state: Box<dyn Fn(&StateFrame<'_>) + Send + Sync>,
    pub recorded: Box<dyn Fn(Recorded) + Send + Sync>,
    /// `(session, plugin id, the params' JSON text)`: a plugin's editor closed.
    pub params: ParamsSink,
    /// `(session, plugin id, every parameter)`: a watched value moved.
    pub params_live: ParamsLiveSink,
}

fn sinks<'a>(ctx: *mut c_void) -> &'a StateSinks {
    unsafe { &*ctx.cast::<StateSinks>() }
}

unsafe extern "C" fn state_trampoline(ctx: *mut c_void, frame: *const K7StateFrame) {
    let frame = StateFrame(unsafe { &*frame });
    let _ = catch_unwind(AssertUnwindSafe(|| (sinks(ctx).state)(&frame)));
}

unsafe extern "C" fn recorded_trampoline(
    ctx: *mut c_void,
    session: K7Str,
    track: K7Str,
    file: K7Str,
    start: f64,
    length: f64,
) {
    let take = Recorded {
        session: text(session),
        track: text(track),
        file: text(file),
        start,
        length,
    };
    let _ = catch_unwind(AssertUnwindSafe(|| (sinks(ctx).recorded)(take)));
}

unsafe extern "C" fn params_trampoline(ctx: *mut c_void, session: K7Str, id: K7Str, json: K7Str) {
    let (session, id, json) = (text(session), text(id), text(json));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        (sinks(ctx).params)(&session, &id, &json)
    }));
}

unsafe extern "C" fn live_trampoline(
    ctx: *mut c_void,
    session: K7Str,
    id: K7Str,
    params: *const K7ParamValue,
    count: usize,
) {
    let values: Vec<(String, f64)> = unsafe { lent(params, count) }
        .iter()
        .map(|p| (text(p.name), p.value))
        .collect();
    let (session, id) = (text(session), text(id));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        (sinks(ctx).params_live)(&session, &id, &values)
    }));
}

unsafe extern "C" fn info_collector(ctx: *mut c_void, info: *const K7ParamInfo) {
    let out = unsafe { &mut *ctx.cast::<Vec<ParamInfo>>() };
    let i = unsafe { &*info };
    out.push(ParamInfo {
        name: text(i.name),
        value: i.value,
        min: i.min,
        max: i.max,
        default: (i.has_default != 0).then_some(i.def),
        states: unsafe { lent(i.states, i.state_count) }
            .iter()
            .map(|s| (text(s.label), s.value))
            .collect(),
    });
}

impl Api {
    /// The state area, `None` when the engine has not built it.
    pub fn state(&self) -> Option<&'static K7StateApi> {
        let ptr = self.table().state.cast::<K7StateApi>();
        (!ptr.is_null()).then(|| unsafe { &*ptr })
    }
}

impl Mt<'_> {
    /// Starts the state timer at `interval_ms` and installs `sinks`, leaked: one per run, as the engine keeps them.
    pub fn start_state(&self, sinks: StateSinks, interval_ms: i32) {
        let Some(area) = self.api().state() else {
            return;
        };
        let ctx = (Box::leak(Box::new(sinks)) as *mut StateSinks).cast::<c_void>();
        let table = K7StateSinks {
            ctx,
            state: Some(state_trampoline),
            recorded: Some(recorded_trampoline),
            params: Some(params_trampoline),
            params_live: Some(live_trampoline),
        };
        unsafe { (area.start)(self.host(), &table, interval_ms) };
    }

    /// A new subscriber holds none of the take being recorded: the next frame carries the whole picture.
    pub fn rewind_recorder_envelopes(&self, session: &str) {
        if let Some(area) = self.api().state() {
            unsafe { (area.rewind_recorder_envelopes)(self.host(), K7Str::of(session)) };
        }
    }

    pub fn watch_plugin_params(&self, session: &str, id: &str, on: bool) {
        if let Some(area) = self.api().state() {
            unsafe {
                (area.watch_plugin_params)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(id),
                    i32::from(on),
                )
            };
        }
    }

    /// Every automatable parameter of a live plugin with its current value; none when there is no such plugin.
    pub fn plugin_param_infos(&self, session: &str, id: &str) -> Vec<ParamInfo> {
        let mut out: Vec<ParamInfo> = Vec::new();
        if let Some(area) = self.api().state() {
            unsafe {
                (area.plugin_param_infos)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(id),
                    (&mut out as *mut Vec<ParamInfo>).cast(),
                    Some(info_collector),
                )
            };
        }
        out
    }
}
