//! machine.rs — the machine area of the C ABI (`k7_engine_machine.h`) made safe: the sessions the engine holds and
//! who drives them, its audio interface and inputs, the audio callback's cost and the master tap the broadcast drains.
//! What a reader gets wrong: `drain_broadcast` and `skip_broadcast_to_live` are for ONE thread (the tap's ring is
//! single-consumer); an engine without this area answers nothing, as it does without the project area.

use super::ffi::{K7Host, K7Str, table};
use super::{Api, Engine, Mt};
use std::ffi::c_void;

#[repr(C)]
pub struct K7SessionInfo {
    pub id: K7Str,
    pub project: K7Str,
    pub loaded: i32,
    pub playing: i32,
    pub track_count: i32,
}

#[repr(C)]
pub struct K7AudioDevice {
    pub name: K7Str,
    pub ty: K7Str,
    pub inputs: i32,
    pub outputs: i32,
    pub current: i32,
    pub current_input: i32,
    pub is_default: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct K7IntList {
    pub ptr: *const i32,
    pub len: usize,
}

#[repr(C)]
pub struct K7ChannelLevel {
    pub db: f32,
    pub found: i32,
}

#[repr(C)]
pub struct K7ChannelPitch {
    pub hz: f32,
    pub clarity: f32,
}

#[repr(C)]
pub struct K7InputDevice {
    pub name: K7Str,
    pub channels: *const i32,
    pub channel_count: usize,
    pub enabled: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct K7CallbackCost {
    pub mean_seconds: f64,
    pub max_seconds: f64,
    pub min_seconds: f64,
    pub runs: i64,
    pub deadline_seconds: f64,
}

#[repr(C)]
pub struct K7BroadcastInfo {
    pub frames: usize,
    pub lost_frames: usize,
    pub sample_rate: f64,
}

pub type K7SessionEach = Option<unsafe extern "C" fn(ctx: *mut c_void, info: *const K7SessionInfo)>;
pub type K7AudioDeviceEach =
    Option<unsafe extern "C" fn(ctx: *mut c_void, device: *const K7AudioDevice)>;
pub type K7ChannelLevelEach =
    Option<unsafe extern "C" fn(ctx: *mut c_void, level: *const K7ChannelLevel)>;
pub type K7ChannelPitchEach =
    Option<unsafe extern "C" fn(ctx: *mut c_void, pitch: *const K7ChannelPitch)>;
pub type K7InputDeviceEach =
    Option<unsafe extern "C" fn(ctx: *mut c_void, device: *const K7InputDevice)>;

table! {
    /// `k7_engine_machine.h`.
    K7MachineApi {
        callback_cost: unsafe extern "C" fn(host: *mut K7Host) -> K7CallbackCost,
        reset_callback_cost: unsafe extern "C" fn(host: *mut K7Host),
        instrument_installed: unsafe extern "C" fn(reference: K7Str) -> i32,
        drain_broadcast: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, max_frames: usize, samples: *mut f32, info: *mut K7BroadcastInfo),
        skip_broadcast_to_live: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        add_driver: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        remove_driver: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        enable_broadcast: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, on: i32),
        silence_output: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, on: i32),
        session_infos: unsafe extern "C" fn(host: *mut K7Host, ctx: *mut c_void, each: K7SessionEach),
        drop_session: unsafe extern "C" fn(host: *mut K7Host, session: K7Str),
        list_audio_devices: unsafe extern "C" fn(host: *mut K7Host, ctx: *mut c_void, each: K7AudioDeviceEach),
        set_audio_device: unsafe extern "C" fn(host: *mut K7Host, name: K7Str, ty: K7Str) -> i32,
        channel_levels_db: unsafe extern "C" fn(host: *mut K7Host, sets: *const K7IntList, set_count: usize, ctx: *mut c_void, each: K7ChannelLevelEach),
        channel_pitches: unsafe extern "C" fn(host: *mut K7Host, sets: *const K7IntList, set_count: usize, ctx: *mut c_void, each: K7ChannelPitchEach),
        input_devices: unsafe extern "C" fn(host: *mut K7Host, ctx: *mut c_void, each: K7InputDeviceEach),
        set_input_stereo: unsafe extern "C" fn(host: *mut K7Host, channel: i32, stereo: i32) -> i32,
        master_level_db: unsafe extern "C" fn(host: *mut K7Host, session: K7Str) -> f32,
    }
}

impl Api {
    /// The machine area, `None` while the engine has not built it.
    pub fn machine(&self) -> Option<&'static K7MachineApi> {
        let p = self.table().machine.cast::<K7MachineApi>();
        unsafe { p.as_ref() }
    }
}

/// One session as `list-sessions` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub project: String,
    pub loaded: bool,
    pub playing: bool,
    pub tracks: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub ty: String,
    pub inputs: i32,
    pub outputs: i32,
    pub current: bool,
    pub current_input: bool,
    pub is_default: bool,
}

/// One channel set's peak in dB; `found` is false when no device of the rig carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelLevel {
    pub db: f32,
    pub found: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelPitch {
    pub hz: f32,
    pub clarity: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputDevice {
    pub name: String,
    pub channels: Vec<i32>,
    pub enabled: bool,
}

pub type CallbackCost = K7CallbackCost;

/// What a drain answered with; its samples are in the buffer the caller lent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drained {
    pub frames: usize,
    pub lost_frames: usize,
    pub sample_rate: f64,
}

unsafe extern "C" fn push_session(ctx: *mut c_void, info: *const K7SessionInfo) {
    let out = unsafe { &mut *ctx.cast::<Vec<SessionInfo>>() };
    let i = unsafe { &*info };
    out.push(unsafe {
        SessionInfo {
            id: i.id.to_string_lossy(),
            project: i.project.to_string_lossy(),
            loaded: i.loaded != 0,
            playing: i.playing != 0,
            tracks: i.track_count,
        }
    });
}

unsafe extern "C" fn push_device(ctx: *mut c_void, device: *const K7AudioDevice) {
    let out = unsafe { &mut *ctx.cast::<Vec<AudioDevice>>() };
    let d = unsafe { &*device };
    out.push(unsafe {
        AudioDevice {
            name: d.name.to_string_lossy(),
            ty: d.ty.to_string_lossy(),
            inputs: d.inputs,
            outputs: d.outputs,
            current: d.current != 0,
            current_input: d.current_input != 0,
            is_default: d.is_default != 0,
        }
    });
}

unsafe extern "C" fn push_level(ctx: *mut c_void, level: *const K7ChannelLevel) {
    let out = unsafe { &mut *ctx.cast::<Vec<ChannelLevel>>() };
    let l = unsafe { &*level };
    out.push(ChannelLevel {
        db: l.db,
        found: l.found != 0,
    });
}

unsafe extern "C" fn push_pitch(ctx: *mut c_void, pitch: *const K7ChannelPitch) {
    let out = unsafe { &mut *ctx.cast::<Vec<ChannelPitch>>() };
    let p = unsafe { &*pitch };
    out.push(ChannelPitch {
        hz: p.hz,
        clarity: p.clarity,
    });
}

unsafe extern "C" fn push_input(ctx: *mut c_void, device: *const K7InputDevice) {
    let out = unsafe { &mut *ctx.cast::<Vec<InputDevice>>() };
    let d = unsafe { &*device };
    let channels = if d.channels.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(d.channels, d.channel_count) }.to_vec()
    };
    out.push(unsafe {
        InputDevice {
            name: d.name.to_string_lossy(),
            channels,
            enabled: d.enabled != 0,
        }
    });
}

/// The channel sets as the ABI's lists, borrowed from `sets` for one call.
fn lists(sets: &[Vec<i32>]) -> Vec<K7IntList> {
    sets.iter()
        .map(|s| K7IntList {
            ptr: s.as_ptr(),
            len: s.len(),
        })
        .collect()
}

impl Engine {
    /// ANY THREAD. What the audio callback has cost since the last reset; `None` without the area or the host.
    pub fn callback_cost(&self) -> Option<CallbackCost> {
        let m = self.api().machine()?;
        let host = self.raw_locked()?;
        Some(unsafe { (m.callback_cost)(host) })
    }

    /// ANY THREAD. Clears the callback's cost window, to measure one gesture.
    pub fn reset_callback_cost(&self) {
        if let (Some(m), Some(host)) = (self.api().machine(), self.raw_locked()) {
            unsafe { (m.reset_callback_cost)(host) }
        }
    }

    /// ANY THREAD. Whether the instrument `reference` can load on this machine: file stats only.
    pub fn instrument_installed(&self, reference: &str) -> bool {
        self.api()
            .machine()
            .is_some_and(|m| unsafe { (m.instrument_installed)(K7Str::of(reference)) != 0 })
    }

    /// THE BROADCAST SENDER'S THREAD ONLY. Takes up to `max_frames` stereo frames of the session's tap into
    /// `samples`, which is resized to what came; an engine without the area answers an empty chunk.
    pub fn drain_broadcast(
        &self,
        session: &str,
        max_frames: usize,
        samples: &mut Vec<f32>,
    ) -> Drained {
        samples.clear();
        let (Some(m), Some(host)) = (self.api().machine(), self.raw_locked()) else {
            return Drained::default();
        };
        samples.resize(max_frames * 2, 0.0);
        let mut info = K7BroadcastInfo {
            frames: 0,
            lost_frames: 0,
            sample_rate: 0.0,
        };
        unsafe {
            (m.drain_broadcast)(
                host,
                K7Str::of(session),
                max_frames,
                samples.as_mut_ptr(),
                &mut info,
            )
        };
        let frames = info.frames.min(max_frames);
        samples.truncate(frames * 2);
        Drained {
            frames,
            lost_frames: info.lost_frames,
            sample_rate: info.sample_rate,
        }
    }

    /// THE BROADCAST SENDER'S THREAD ONLY. A listener who just arrived hears now, not the ring's backlog.
    pub fn skip_broadcast_to_live(&self, session: &str) {
        if let (Some(m), Some(host)) = (self.api().machine(), self.raw_locked()) {
            unsafe { (m.skip_broadcast_to_live)(host, K7Str::of(session)) }
        }
    }
}

impl Mt<'_> {
    fn machine(&self) -> Option<&'static K7MachineApi> {
        self.api().machine()
    }

    /// A socket driving `session` arrived.
    pub fn add_driver(&self, session: &str) {
        if let Some(m) = self.machine() {
            unsafe { (m.add_driver)(self.host(), K7Str::of(session)) }
        }
    }

    pub fn remove_driver(&self, session: &str) {
        if let Some(m) = self.machine() {
            unsafe { (m.remove_driver)(self.host(), K7Str::of(session)) }
        }
    }

    /// The tap on or off; on creates the session, off never does.
    pub fn enable_broadcast(&self, session: &str, on: bool) {
        if let Some(m) = self.machine() {
            unsafe { (m.enable_broadcast)(self.host(), K7Str::of(session), i32::from(on)) }
        }
    }

    /// Keeps the session's mix from the interface while still broadcasting it.
    pub fn silence_output(&self, session: &str, on: bool) {
        if let Some(m) = self.machine() {
            unsafe { (m.silence_output)(self.host(), K7Str::of(session), i32::from(on)) }
        }
    }

    pub fn session_infos(&self) -> Vec<SessionInfo> {
        let mut out: Vec<SessionInfo> = Vec::new();
        if let Some(m) = self.machine() {
            let ctx = (&mut out as *mut Vec<SessionInfo>).cast();
            unsafe { (m.session_infos)(self.host(), ctx, Some(push_session)) }
        }
        out
    }

    pub fn drop_session(&self, session: &str) {
        if let Some(m) = self.machine() {
            unsafe { (m.drop_session)(self.host(), K7Str::of(session)) }
        }
    }

    /// Every interface, ASIO first.
    pub fn list_audio_devices(&self) -> Vec<AudioDevice> {
        let mut out: Vec<AudioDevice> = Vec::new();
        if let Some(m) = self.machine() {
            let ctx = (&mut out as *mut Vec<AudioDevice>).cast();
            unsafe { (m.list_audio_devices)(self.host(), ctx, Some(push_device)) }
        }
        out
    }

    /// Opens an interface for input and output; false when none by that name (and `ty`, empty for any backend).
    pub fn set_audio_device(&self, name: &str, ty: &str) -> bool {
        self.machine().is_some_and(|m| unsafe {
            (m.set_audio_device)(self.host(), K7Str::of(name), K7Str::of(ty)) != 0
        })
    }

    /// One peak per channel set, in order: the interface's, not the song's.
    pub fn channel_levels_db(&self, sets: &[Vec<i32>]) -> Vec<ChannelLevel> {
        let mut out: Vec<ChannelLevel> = Vec::new();
        if let Some(m) = self.machine() {
            let lists = lists(sets);
            let ctx = (&mut out as *mut Vec<ChannelLevel>).cast();
            unsafe {
                (m.channel_levels_db)(
                    self.host(),
                    lists.as_ptr(),
                    lists.len(),
                    ctx,
                    Some(push_level),
                )
            }
        }
        out
    }

    /// One pitch per channel set, in order.
    pub fn channel_pitches(&self, sets: &[Vec<i32>]) -> Vec<ChannelPitch> {
        let mut out: Vec<ChannelPitch> = Vec::new();
        if let Some(m) = self.machine() {
            let lists = lists(sets);
            let ctx = (&mut out as *mut Vec<ChannelPitch>).cast();
            unsafe {
                (m.channel_pitches)(
                    self.host(),
                    lists.as_ptr(),
                    lists.len(),
                    ctx,
                    Some(push_pitch),
                )
            }
        }
        out
    }

    /// How the rig groups its inputs: one entry per wave input device.
    pub fn input_devices(&self) -> Vec<InputDevice> {
        let mut out: Vec<InputDevice> = Vec::new();
        if let Some(m) = self.machine() {
            let ctx = (&mut out as *mut Vec<InputDevice>).cast();
            unsafe { (m.input_devices)(self.host(), ctx, Some(push_input)) }
        }
        out
    }

    /// Pairs the input carrying `channel` (1-based) with its partner or splits it; false with no interface open.
    pub fn set_input_stereo(&self, channel: i32, stereo: bool) -> bool {
        self.machine().is_some_and(|m| unsafe {
            (m.set_input_stereo)(self.host(), channel, i32::from(stereo)) != 0
        })
    }

    /// Peak dB leaving the session's master, -100 with no edit loaded.
    pub fn master_level_db(&self, session: &str) -> f32 {
        self.machine().map_or(-100.0, |m| unsafe {
            (m.master_level_db)(self.host(), K7Str::of(session))
        })
    }
}
