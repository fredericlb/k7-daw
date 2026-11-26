//! fake_plugins.rs — the plugins area of the fake engine: a catalogue, a scan and probes a test scripts and can
//! hold shut, instances and presets per session, and a log of the message-thread calls made.
//! What a reader gets wrong: the area is process-wide, as the real catalogue and its probes are, so a test takes
//! `serial()` and `reset()`s; a held scan or probe waits on a condvar until `release()`, and its in-flight peak is counted.

use super::fake::with_host_state;
use super::ffi::{K7Host, K7Str, K7StrEach};
use super::plugins::{
    K7InstanceEach, K7PluginEach, K7PluginInfo, K7PluginInstance, K7PluginsApi, K7Preset,
    K7PresetEach, K7ScanStep, PluginInfo, PluginInstance, Preset,
};
use super::state::{K7ParamInfo, K7ParamInfoEach, K7ParamState, ParamInfo};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};

/// What the tests set and read of the area.
#[derive(Default)]
pub struct Fake {
    pub cache: Vec<PluginInfo>,
    pub warm: bool,
    /// What a scan ends on, or the reason it threw.
    pub scan_result: Option<Result<Vec<PluginInfo>, String>>,
    /// `(done, total, loading)` each scan reports before it ends.
    pub scan_steps: Vec<(i32, i32, String)>,
    pub hold_scans: bool,
    pub hold_probes: bool,
    pub scans: Vec<bool>,
    pub scans_running: usize,
    pub scans_peak: usize,
    /// What a type's probe answers; a type not listed could not be loaded.
    pub specs: HashMap<(String, String), Vec<ParamInfo>>,
    pub probes: Vec<String>,
    pub probes_running: usize,
    pub probes_peak: usize,
    /// Which `(format, id)` load for a window and then open.
    pub loadable: Vec<(String, String)>,
    pub opens_ok: bool,
    pub snapshot_ok: bool,
    pub instances: HashMap<String, Vec<PluginInstance>>,
    pub presets: HashMap<(String, String), Vec<Preset>>,
    /// The message-thread calls, in order.
    pub calls: Vec<String>,
    /// Message-thread functions called from another thread.
    pub off_thread: usize,
}

struct Shared {
    state: Mutex<Fake>,
    wake: Condvar,
    serial: Mutex<()>,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        state: Mutex::new(Fake::default()),
        wake: Condvar::new(),
        serial: Mutex::new(()),
    })
}

fn lock() -> MutexGuard<'static, Fake> {
    shared().state.lock().unwrap_or_else(|e| e.into_inner())
}

/// Held by a test for its whole run: the area is the process's, so two tests cannot share it.
pub fn serial() -> MutexGuard<'static, ()> {
    shared().serial.lock().unwrap_or_else(|e| e.into_inner())
}

/// Everything back to a quiet engine: nothing installed, nothing held.
pub fn reset() {
    *lock() = Fake::default();
    shared().wake.notify_all();
}

/// Reads and changes what the area holds.
pub fn with<R>(f: impl FnOnce(&mut Fake) -> R) -> R {
    let out = f(&mut lock());
    shared().wake.notify_all();
    out
}

/// Lets every held scan and probe go.
pub fn release() {
    with(|f| {
        f.hold_scans = false;
        f.hold_probes = false;
    });
}

/// Waits (unlocking) while `held` says so, with `running` counted in and out around the wait.
fn hold<'a>(mut g: MutexGuard<'a, Fake>, held: impl Fn(&Fake) -> bool) -> MutexGuard<'a, Fake> {
    while held(&g) {
        g = shared().wake.wait(g).unwrap_or_else(|e| e.into_inner());
    }
    g
}

pub static PLUGINS: K7PluginsApi = K7PluginsApi {
    cache_warm,
    load_cache,
    scan,
    plugin_params_of,
    prepare_native_ui,
    show_native_ui_of,
    snapshot_orphan_to,
    show_native_ui,
    plugin_instances,
    list_presets,
    set_preset,
    set_plugin_param,
    enable_waveform_tap,
    disable_waveform_tap,
};

fn text(s: K7Str) -> String {
    unsafe { s.to_string_lossy() }
}

/// Logs a message-thread call, and counts it when it did not run on the thread that runs the host.
fn on_message_thread(host: *mut K7Host, call: String) {
    let run = with_host_state(host, |s| s.run_thread).flatten();
    let wrong = run.is_some_and(|t| t != std::thread::current().id());
    let mut g = lock();
    g.off_thread += usize::from(wrong);
    g.calls.push(call);
}

fn give_plugins(list: &[PluginInfo], ctx: *mut c_void, each: K7PluginEach) {
    let Some(each) = each else { return };
    for p in list {
        let raw = K7PluginInfo {
            name: K7Str::of(&p.name),
            format: K7Str::of(&p.format),
            manufacturer: K7Str::of(&p.manufacturer),
            instrument: i32::from(p.instrument),
            role: K7Str::of(&p.role),
            category: K7Str::of(&p.category),
            id: K7Str::of(&p.id),
        };
        unsafe { each(ctx, &raw) };
    }
}

unsafe extern "C" fn cache_warm() -> i32 {
    i32::from(lock().warm)
}

unsafe extern "C" fn load_cache(ctx: *mut c_void, each: K7PluginEach) {
    let cache = lock().cache.clone();
    give_plugins(&cache, ctx, each);
}

unsafe extern "C" fn scan(
    force: i32,
    ctx: *mut c_void,
    step: K7ScanStep,
    each: K7PluginEach,
    failed: K7StrEach,
) -> i32 {
    let steps = {
        let mut g = lock();
        g.scans.push(force != 0);
        g.scans_running += 1;
        g.scans_peak = g.scans_peak.max(g.scans_running);
        g.scan_steps.clone()
    };
    if let Some(step) = step {
        for (done, total, loading) in &steps {
            unsafe { step(ctx, *done, *total, K7Str::of(loading)) };
        }
    }
    let g = hold(lock(), |f| f.hold_scans);
    let result = g.scan_result.clone();
    let mut g = g;
    g.scans_running -= 1;
    drop(g);
    match result {
        Some(Err(why)) => {
            if let Some(failed) = failed {
                unsafe { failed(ctx, K7Str::of(&why)) };
            }
            0
        }
        Some(Ok(found)) => {
            give_plugins(&found, ctx, each);
            1
        }
        None => 1,
    }
}

/// The wait and the count both probes share.
fn probing(what: String) -> MutexGuard<'static, Fake> {
    let mut g = lock();
    g.probes.push(what);
    g.probes_running += 1;
    g.probes_peak = g.probes_peak.max(g.probes_running);
    let mut g = hold(g, |f| f.hold_probes);
    g.probes_running -= 1;
    g
}

unsafe extern "C" fn plugin_params_of(
    format: K7Str,
    id: K7Str,
    ctx: *mut c_void,
    each: K7ParamInfoEach,
) -> i32 {
    let key = (text(format), text(id));
    let found = probing(format!("params {} {}", key.0, key.1))
        .specs
        .get(&key)
        .cloned();
    let Some(infos) = found else { return 0 };
    if let Some(each) = each {
        for p in &infos {
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
                value: 0.0,
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
    1
}

unsafe extern "C" fn prepare_native_ui(format: K7Str, id: K7Str) -> i32 {
    let key = (text(format), text(id));
    let g = probing(format!("prepare {} {}", key.0, key.1));
    i32::from(g.loadable.contains(&key))
}

unsafe extern "C" fn show_native_ui_of(host: *mut K7Host, format: K7Str, id: K7Str) -> i32 {
    let key = (text(format), text(id));
    on_message_thread(host, format!("show_native_ui_of {} {}", key.0, key.1));
    let g = lock();
    i32::from(g.opens_ok && g.loadable.contains(&key))
}

unsafe extern "C" fn snapshot_orphan_to(
    host: *mut K7Host,
    format: K7Str,
    id: K7Str,
    path: K7Str,
) -> i32 {
    on_message_thread(
        host,
        format!(
            "snapshot_orphan_to {} {} {}",
            text(format),
            text(id),
            text(path)
        ),
    );
    i32::from(lock().snapshot_ok)
}

unsafe extern "C" fn show_native_ui(host: *mut K7Host, session: K7Str, id: K7Str) {
    on_message_thread(
        host,
        format!("show_native_ui {} {}", text(session), text(id)),
    );
}

unsafe extern "C" fn plugin_instances(
    host: *mut K7Host,
    session: K7Str,
    ctx: *mut c_void,
    each: K7InstanceEach,
) {
    let session = text(session);
    on_message_thread(host, format!("plugin_instances {session}"));
    let list = lock().instances.get(&session).cloned().unwrap_or_default();
    let Some(each) = each else { return };
    for p in &list {
        let raw = K7PluginInstance {
            id: K7Str::of(&p.id),
            name: K7Str::of(&p.name),
            has_native_ui: i32::from(p.has_native_ui),
        };
        unsafe { each(ctx, &raw) };
    }
}

unsafe extern "C" fn list_presets(
    host: *mut K7Host,
    session: K7Str,
    module: K7Str,
    ctx: *mut c_void,
    each: K7PresetEach,
) {
    let key = (text(session), text(module));
    on_message_thread(host, format!("list_presets {} {}", key.0, key.1));
    let list = lock().presets.get(&key).cloned().unwrap_or_default();
    let Some(each) = each else { return };
    for p in &list {
        let raw = K7Preset {
            name: K7Str::of(&p.name),
            source: K7Str::of(&p.source),
            index: p.index,
            path: K7Str::of(&p.path),
        };
        unsafe { each(ctx, &raw) };
    }
}

unsafe extern "C" fn set_preset(
    host: *mut K7Host,
    session: K7Str,
    module: K7Str,
    index: i32,
    path: K7Str,
) {
    on_message_thread(
        host,
        format!(
            "set_preset {} {} {index} {}",
            text(session),
            text(module),
            text(path)
        ),
    );
}

unsafe extern "C" fn set_plugin_param(
    host: *mut K7Host,
    session: K7Str,
    id: K7Str,
    name: K7Str,
    value: f64,
) {
    on_message_thread(
        host,
        format!(
            "set_plugin_param {} {} {} {value}",
            text(session),
            text(id),
            text(name)
        ),
    );
}

unsafe extern "C" fn enable_waveform_tap(
    host: *mut K7Host,
    session: K7Str,
    id: K7Str,
    side: K7Str,
) {
    on_message_thread(
        host,
        format!("tap_on {} {} {}", text(session), text(id), text(side)),
    );
}

unsafe extern "C" fn disable_waveform_tap(
    host: *mut K7Host,
    session: K7Str,
    id: K7Str,
    side: K7Str,
) {
    on_message_thread(
        host,
        format!("tap_off {} {} {}", text(session), text(id), text(side)),
    );
}
