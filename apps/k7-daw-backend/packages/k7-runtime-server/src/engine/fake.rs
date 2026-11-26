//! fake.rs — an engine with no Tracktion behind it: the C ABI's tables implemented in Rust, a message loop on
//! whichever thread runs the host, and a log of the calls made. The engine the server's tests run against;
//! each area's milestone adds its fake functions here beside its table.
//! What a reader gets wrong: it keeps the real contract, not a convenient one: a job runs on the thread that
//! runs the host, a quit ends the loop after the job running, and a job left queued is dropped at destroy.

pub mod machine;
pub mod project;
pub mod render;
pub mod transport;

use super::Engine;
use super::fake_plugins;
use super::fake_state;
use super::ffi::{
    self, K7AudioTiming, K7CoreApi, K7EngineApi, K7Host, K7Str, K7StrEach, K7Task, K7Warning,
    K7WarningSink,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread::ThreadId;

/// The fake's table, built once.
pub fn api() -> &'static K7EngineApi {
    static TABLE: OnceLock<K7EngineApi> = OnceLock::new();
    TABLE.get_or_init(|| K7EngineApi {
        abi: ffi::K7_ENGINE_ABI,
        size: std::mem::size_of::<K7EngineApi>() as u32,
        core: &CORE,
        project: std::ptr::from_ref(&project::TABLE).cast(),
        transport: std::ptr::from_ref(&transport::TABLE).cast(),
        state: (&fake_state::STATE as *const super::state::K7StateApi).cast(),
        plugins: (&fake_plugins::PLUGINS as *const super::plugins::K7PluginsApi).cast(),
        machine: std::ptr::from_ref(&machine::TABLE).cast(),
        render: std::ptr::from_ref(&render::TABLE).cast(),
        cli: std::ptr::null(),
    })
}

/// What the tests read and set on one fake host.
#[derive(Default)]
pub struct FakeState {
    pub calls: Vec<String>,
    pub sessions: Vec<String>,
    pub timing: K7AudioTiming,
    pub output_device: String,
    pub run_thread: Option<ThreadId>,
    pub project: project::ProjectFake,
    pub transport: transport::TransportFake,
    pub state_area: fake_state::Area,
    pub machine: machine::MachineFake,
}

struct Queued {
    run: K7Task,
    drop: K7Task,
    ctx: *mut c_void,
}

// A job's context is the server's boxed closure, which is Send.
unsafe impl Send for Queued {}

#[derive(Default)]
struct FakeHost {
    queue: Mutex<(VecDeque<Queued>, bool)>,
    wake: Condvar,
    state: Mutex<FakeState>,
}

fn fake(host: *const K7Host) -> &'static FakeHost {
    unsafe { &*host.cast::<FakeHost>() }
}

/// The state of the fake host behind `engine`, read and changed under its lock; `None` once destroyed.
pub fn with_state<R>(engine: &Engine, f: impl FnOnce(&mut FakeState) -> R) -> Option<R> {
    let host = engine.raw()?;
    let mut state = fake(host).state.lock().ok()?;
    Some(f(&mut state))
}

pub(super) fn log(host: *const K7Host, call: &str) {
    if let Ok(mut s) = fake(host).state.lock() {
        s.calls.push(call.to_string());
    }
}

/// An area's own state of a fake host, read and changed under its lock; `None` if it was poisoned.
pub(super) fn with_host_state<R>(
    host: *const K7Host,
    f: impl FnOnce(&mut FakeState) -> R,
) -> Option<R> {
    let mut state = fake(host).state.lock().ok()?;
    Some(f(&mut state))
}

struct Process {
    settings_dir: String,
    safe_mode: bool,
    sink: Option<(K7WarningSink, usize)>,
}

static PROCESS: Mutex<Process> = Mutex::new(Process {
    settings_dir: String::new(),
    safe_mode: false,
    sink: None,
});

/// Says a warning through the sink the server installed, as the engine's warn() does.
pub fn warn(session: &str, subject: &str, message: &str) {
    let sink = PROCESS.lock().ok().and_then(|p| p.sink);
    if let Some((Some(sink), ctx)) = sink {
        let w = K7Warning {
            session: K7Str::of(session),
            subject: K7Str::of(subject),
            message: K7Str::of(message),
        };
        unsafe { sink(ctx as *mut c_void, &w) };
    }
}

/// Whether the server put the process in safe mode.
pub fn safe_mode() -> bool {
    PROCESS.lock().map(|p| p.safe_mode).unwrap_or(false)
}

fn give(out: K7StrEach, ctx: *mut c_void, s: &str) {
    if let Some(out) = out {
        unsafe { out(ctx, K7Str::of(s)) };
    }
}

unsafe extern "C" fn layout_consistent() -> i32 {
    1
}

unsafe extern "C" fn install_crash_handler() {}

unsafe extern "C" fn init_settings_dir(flag: K7Str) {
    let flag = unsafe { flag.to_string_lossy() };
    let env = std::env::var("K7_SETTINGS_DIR").unwrap_or_default();
    if let Ok(mut p) = PROCESS.lock() {
        p.settings_dir = if flag.is_empty() { env } else { flag };
    }
}

unsafe extern "C" fn settings_dir(ctx: *mut c_void, out: K7StrEach) {
    let dir = PROCESS
        .lock()
        .map(|p| p.settings_dir.clone())
        .unwrap_or_default();
    give(out, ctx, &dir);
}

unsafe extern "C" fn state_path(ctx: *mut c_void, out: K7StrEach) {
    give(
        out,
        ctx,
        &std::env::temp_dir()
            .join("k7-fake-engine")
            .to_string_lossy(),
    );
}

unsafe extern "C" fn enable_load_guards() {}

unsafe extern "C" fn set_safe_mode(on: i32) {
    if let Ok(mut p) = PROCESS.lock() {
        p.safe_mode = on != 0;
    }
}

unsafe extern "C" fn use_default_path_roots() {}

unsafe extern "C" fn set_warning_sink(sink: K7WarningSink, ctx: *mut c_void) {
    if let Ok(mut p) = PROCESS.lock() {
        p.sink = sink.map(|s| (Some(s), ctx as usize));
    }
}

unsafe extern "C" fn host_create() -> *mut K7Host {
    Box::into_raw(Box::<FakeHost>::default()).cast()
}

unsafe extern "C" fn host_destroy(host: *mut K7Host) {
    let host = unsafe { Box::from_raw(host.cast::<FakeHost>()) };
    let left: Vec<Queued> = host
        .queue
        .lock()
        .map(|mut q| q.0.drain(..).collect())
        .unwrap_or_default();
    for job in left {
        if let Some(drop) = job.drop {
            unsafe { drop(job.ctx) };
        }
    }
}

unsafe extern "C" fn run(host: *mut K7Host) {
    let h = fake(host);
    if let Ok(mut s) = h.state.lock() {
        s.run_thread = Some(std::thread::current().id());
    }
    loop {
        let job = {
            let Ok(mut q) = h.queue.lock() else { return };
            loop {
                if q.1 {
                    q.1 = false;
                    return;
                }
                if let Some(job) = q.0.pop_front() {
                    break job;
                }
                q = match h.wake.wait(q) {
                    Ok(q) => q,
                    Err(_) => return,
                };
            }
        };
        if let Some(run) = job.run {
            unsafe { run(job.ctx) };
        }
    }
}

unsafe extern "C" fn quit(host: *mut K7Host) {
    let h = fake(host);
    if let Ok(mut q) = h.queue.lock() {
        q.1 = true;
    }
    h.wake.notify_all();
}

unsafe extern "C" fn post(host: *mut K7Host, run: K7Task, drop: K7Task, ctx: *mut c_void) {
    let h = fake(host);
    if let Ok(mut q) = h.queue.lock() {
        q.0.push_back(Queued { run, drop, ctx });
    }
    h.wake.notify_all();
}

unsafe extern "C" fn open_audio_device(host: *mut K7Host) {
    log(host, "open_audio_device");
}

unsafe extern "C" fn output_device_name(host: *mut K7Host, ctx: *mut c_void, out: K7StrEach) {
    let name = fake(host)
        .state
        .lock()
        .map(|s| s.output_device.clone())
        .unwrap_or_default();
    give(out, ctx, &name);
}

unsafe extern "C" fn suspend_audio(host: *mut K7Host, suspended: i32) {
    log(host, &format!("suspend_audio {suspended}"));
}

unsafe extern "C" fn audio_timing(host: *const K7Host) -> K7AudioTiming {
    fake(host)
        .state
        .lock()
        .map(|s| s.timing)
        .unwrap_or_default()
}

unsafe extern "C" fn message_thread_age_ms(_host: *const K7Host) -> f64 {
    0.0
}

unsafe extern "C" fn session_ids(host: *const K7Host, ctx: *mut c_void, each: K7StrEach) {
    let ids = fake(host)
        .state
        .lock()
        .map(|s| s.sessions.clone())
        .unwrap_or_default();
    for id in ids {
        give(each, ctx, &id);
    }
}

unsafe extern "C" fn has_session(host: *const K7Host, id: K7Str) -> i32 {
    let id = unsafe { id.to_string_lossy() };
    i32::from(
        fake(host)
            .state
            .lock()
            .map(|s| s.sessions.contains(&id))
            .unwrap_or(false),
    )
}

static CORE: K7CoreApi = K7CoreApi {
    layout_consistent,
    install_crash_handler,
    init_settings_dir,
    settings_dir,
    state_path,
    enable_load_guards,
    set_safe_mode,
    use_default_path_roots,
    set_warning_sink,
    host_create,
    host_destroy,
    run,
    quit,
    post,
    open_audio_device,
    output_device_name,
    suspend_audio,
    audio_timing,
    message_thread_age_ms,
    session_ids,
    has_session,
};
