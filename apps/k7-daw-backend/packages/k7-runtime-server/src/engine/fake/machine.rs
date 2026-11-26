//! machine.rs — the fake engine's machine area: sessions and devices a test scripts, a master tap it fills, and
//! every message-thread call written to the host's log as one line, as the transport area does.
//! What a reader gets wrong: the tap drains what a test pushed, `lost` once; and `instrument_installed` has no
//! host to hold its answers, so the installed set is process-wide, which a test keys with a name of its own.

use super::super::ffi::{K7Host, K7Str};
use super::super::machine::{
    AudioDevice, InputDevice, K7AudioDevice, K7AudioDeviceEach, K7BroadcastInfo, K7CallbackCost,
    K7ChannelLevel, K7ChannelLevelEach, K7ChannelPitch, K7ChannelPitchEach, K7InputDevice,
    K7InputDeviceEach, K7IntList, K7MachineApi, K7SessionEach, K7SessionInfo, SessionInfo,
};
use super::{FakeState, fake};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::Mutex;

/// What the master tap of one session holds.
#[derive(Default)]
pub struct Tap {
    /// Interleaved stereo.
    pub samples: Vec<f32>,
    pub sample_rate: f64,
    /// Reported once by the next drain, as the ring's overwritten frames.
    pub lost: usize,
}

/// What a test reads and scripts of the machine area.
#[derive(Default)]
pub struct MachineFake {
    pub infos: Vec<SessionInfo>,
    pub devices: Vec<AudioDevice>,
    pub inputs: Vec<InputDevice>,
    /// Set `n` of a levels or pitches call answers entry `n`; a missing one is silence, not found.
    pub levels: Vec<(f32, bool)>,
    pub pitches: Vec<(f32, f32)>,
    pub master_db: HashMap<String, f32>,
    /// Whether an interface is open to group inputs on.
    pub interface_open: bool,
    pub cost: K7CallbackCost,
    pub taps: HashMap<String, Tap>,
    /// The channel sets of the last levels call.
    pub last_sets: Vec<Vec<i32>>,
}

static INSTALLED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Marks `reference` as an instrument this fake machine has.
pub fn install_instrument(reference: &str) {
    INSTALLED
        .lock()
        .unwrap()
        .get_or_insert_with(HashSet::new)
        .insert(reference.to_string());
}

fn text(s: K7Str) -> String {
    unsafe { s.to_string_lossy() }
}

/// `f` on the host's state under its lock; the call is logged first, as one line.
fn call<R>(host: *const K7Host, line: String, f: impl FnOnce(&mut FakeState) -> R) -> Option<R> {
    fake(host).state.lock().ok().map(|mut s| {
        s.calls.push(line);
        f(&mut s)
    })
}

fn quiet<R>(host: *const K7Host, f: impl FnOnce(&mut FakeState) -> R) -> Option<R> {
    fake(host).state.lock().ok().map(|mut s| f(&mut s))
}

unsafe extern "C" fn callback_cost(host: *mut K7Host) -> K7CallbackCost {
    quiet(host, |s| s.machine.cost).unwrap_or_default()
}

unsafe extern "C" fn reset_callback_cost(host: *mut K7Host) {
    call(host, "reset_callback_cost".into(), |s| {
        s.machine.cost = K7CallbackCost {
            deadline_seconds: s.machine.cost.deadline_seconds,
            ..K7CallbackCost::default()
        }
    });
}

unsafe extern "C" fn instrument_installed(reference: K7Str) -> i32 {
    let r = text(reference);
    i32::from(
        INSTALLED
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|set| set.contains(&r)),
    )
}

unsafe extern "C" fn drain_broadcast(
    host: *mut K7Host,
    session: K7Str,
    max_frames: usize,
    samples: *mut f32,
    info: *mut K7BroadcastInfo,
) {
    let session = text(session);
    let got = quiet(host, |s| {
        let tap = s.machine.taps.entry(session).or_default();
        let frames = (tap.samples.len() / 2).min(max_frames);
        let taken: Vec<f32> = tap.samples.drain(..frames * 2).collect();
        let lost = std::mem::take(&mut tap.lost);
        (taken, frames, lost, tap.sample_rate)
    });
    let Some((taken, frames, lost, rate)) = got else {
        return;
    };
    unsafe {
        std::ptr::copy_nonoverlapping(taken.as_ptr(), samples, taken.len());
        *info = K7BroadcastInfo {
            frames,
            lost_frames: lost,
            sample_rate: rate,
        };
    }
}

unsafe extern "C" fn skip_broadcast_to_live(host: *mut K7Host, session: K7Str) {
    let session = text(session);
    call(host, format!("skip_broadcast_to_live {session}"), |s| {
        if let Some(tap) = s.machine.taps.get_mut(&session) {
            tap.samples.clear();
        }
    });
}

unsafe extern "C" fn add_driver(host: *mut K7Host, session: K7Str) {
    call(host, format!("add_driver {}", text(session)), |_| ());
}

unsafe extern "C" fn remove_driver(host: *mut K7Host, session: K7Str) {
    call(host, format!("remove_driver {}", text(session)), |_| ());
}

unsafe extern "C" fn enable_broadcast(host: *mut K7Host, session: K7Str, on: i32) {
    call(
        host,
        format!("enable_broadcast {} {on}", text(session)),
        |_| (),
    );
}

unsafe extern "C" fn silence_output(host: *mut K7Host, session: K7Str, on: i32) {
    call(
        host,
        format!("silence_output {} {on}", text(session)),
        |_| (),
    );
}

unsafe extern "C" fn session_infos(host: *mut K7Host, ctx: *mut c_void, each: K7SessionEach) {
    let infos = call(host, "session_infos".into(), |s| s.machine.infos.clone()).unwrap_or_default();
    let Some(each) = each else { return };
    for i in &infos {
        let info = K7SessionInfo {
            id: K7Str::of(&i.id),
            project: K7Str::of(&i.project),
            loaded: i32::from(i.loaded),
            playing: i32::from(i.playing),
            track_count: i.tracks,
        };
        unsafe { each(ctx, &info) };
    }
}

unsafe extern "C" fn drop_session(host: *mut K7Host, session: K7Str) {
    let session = text(session);
    call(host, format!("drop_session {session}"), |s| {
        s.machine.infos.retain(|i| i.id != session);
        s.sessions.retain(|i| *i != session);
    });
}

unsafe extern "C" fn list_audio_devices(
    host: *mut K7Host,
    ctx: *mut c_void,
    each: K7AudioDeviceEach,
) {
    let devices =
        call(host, "list_audio_devices".into(), |s| s.machine.devices.clone()).unwrap_or_default();
    let Some(each) = each else { return };
    for d in &devices {
        let device = K7AudioDevice {
            name: K7Str::of(&d.name),
            ty: K7Str::of(&d.ty),
            inputs: d.inputs,
            outputs: d.outputs,
            current: i32::from(d.current),
            current_input: i32::from(d.current_input),
            is_default: i32::from(d.is_default),
        };
        unsafe { each(ctx, &device) };
    }
}

unsafe extern "C" fn set_audio_device(host: *mut K7Host, name: K7Str, ty: K7Str) -> i32 {
    let (name, ty) = (text(name), text(ty));
    let line = format!("set_audio_device {name} [{ty}]");
    i32::from(
        call(host, line, |s| {
            let hit = s
                .machine
                .devices
                .iter()
                .position(|d| d.name == name && (ty.is_empty() || d.ty == ty));
            for (i, d) in s.machine.devices.iter_mut().enumerate() {
                d.current = hit == Some(i);
                d.current_input = hit == Some(i);
            }
            hit.is_some()
        })
        .unwrap_or(false),
    )
}

fn sets_of(sets: *const K7IntList, count: usize) -> Vec<Vec<i32>> {
    (0..count)
        .map(|i| {
            let l = unsafe { *sets.add(i) };
            if l.ptr.is_null() {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(l.ptr, l.len) }.to_vec()
            }
        })
        .collect()
}

unsafe extern "C" fn channel_levels_db(
    host: *mut K7Host,
    sets: *const K7IntList,
    count: usize,
    ctx: *mut c_void,
    each: K7ChannelLevelEach,
) {
    let sets = sets_of(sets, count);
    let levels = call(host, format!("channel_levels_db {sets:?}"), |s| {
        s.machine.last_sets = sets.clone();
        (0..sets.len())
            .map(|i| s.machine.levels.get(i).copied().unwrap_or((-100.0, false)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let Some(each) = each else { return };
    for (db, found) in levels {
        let level = K7ChannelLevel {
            db,
            found: i32::from(found),
        };
        unsafe { each(ctx, &level) };
    }
}

unsafe extern "C" fn channel_pitches(
    host: *mut K7Host,
    sets: *const K7IntList,
    count: usize,
    ctx: *mut c_void,
    each: K7ChannelPitchEach,
) {
    let n = sets_of(sets, count).len();
    let pitches = call(host, "channel_pitches".into(), |s| {
        (0..n)
            .map(|i| s.machine.pitches.get(i).copied().unwrap_or((0.0, 0.0)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let Some(each) = each else { return };
    for (hz, clarity) in pitches {
        let pitch = K7ChannelPitch { hz, clarity };
        unsafe { each(ctx, &pitch) };
    }
}

unsafe extern "C" fn input_devices(host: *mut K7Host, ctx: *mut c_void, each: K7InputDeviceEach) {
    let inputs = quiet(host, |s| s.machine.inputs.clone()).unwrap_or_default();
    let Some(each) = each else { return };
    for d in &inputs {
        let device = K7InputDevice {
            name: K7Str::of(&d.name),
            channels: d.channels.as_ptr(),
            channel_count: d.channels.len(),
            enabled: i32::from(d.enabled),
        };
        unsafe { each(ctx, &device) };
    }
}

unsafe extern "C" fn set_input_stereo(host: *mut K7Host, channel: i32, stereo: i32) -> i32 {
    i32::from(
        call(host, format!("set_input_stereo {channel} {stereo}"), |s| {
            s.machine.interface_open
        })
        .unwrap_or(false),
    )
}

unsafe extern "C" fn master_level_db(host: *mut K7Host, session: K7Str) -> f32 {
    let session = text(session);
    quiet(host, |s| s.machine.master_db.get(&session).copied()).flatten().unwrap_or(-100.0)
}

pub static TABLE: K7MachineApi = K7MachineApi {
    callback_cost,
    reset_callback_cost,
    instrument_installed,
    drain_broadcast,
    skip_broadcast_to_live,
    add_driver,
    remove_driver,
    enable_broadcast,
    silence_output,
    session_infos,
    drop_session,
    list_audio_devices,
    set_audio_device,
    channel_levels_db,
    channel_pitches,
    input_devices,
    set_input_stereo,
    master_level_db,
};
