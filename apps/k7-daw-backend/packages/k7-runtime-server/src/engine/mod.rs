//! engine — the C++ host through its C ABI, made safe: `Api` is the process's table, `Host` the owner on the
//! message thread, `Engine` the handle any task holds, `Mt` what a job running on the message thread is handed.
//! Each ABI area adds a module here with its `impl Mt` or `impl Engine` block, one line in the list below.
//! What a reader gets wrong: an `Engine` may outlive its host; once the host is destroyed a post is dropped and
//! an answer is `None`, never a call into freed memory (`docs/abi.md` beside this crate).

use crate::log::say;
pub mod cli;
pub mod fake;
pub mod fake_plugins;
pub mod fake_state;
pub mod ffi;
pub mod machine;
pub mod plugins;
pub mod project;
pub mod render;
pub mod state;
pub mod transport;

use ffi::{K7CoreApi, K7EngineApi, K7Host, K7Str, K7Warning};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, RwLock};

/// Why a table was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum AbiError {
    Version { engine: u32, server: u32 },
    Short { engine: u32, server: u32 },
    NoCore,
}

/// The engine's table, checked once: the same version, at least as long as this side's, a core area.
#[derive(Clone, Copy)]
pub struct Api(&'static K7EngineApi);

/// An engine warning: `session` empty when it belongs to no project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub session: String,
    pub subject: String,
    pub message: String,
}

pub type WarningSink = Box<dyn Fn(Warning) + Send + Sync>;

/// The interface's timing as the message thread last published it.
pub type AudioTiming = ffi::K7AudioTiming;

impl Api {
    pub fn new(table: &'static K7EngineApi) -> Result<Api, AbiError> {
        if table.abi != ffi::K7_ENGINE_ABI {
            return Err(AbiError::Version {
                engine: table.abi,
                server: ffi::K7_ENGINE_ABI,
            });
        }
        let ours = std::mem::size_of::<K7EngineApi>() as u32;
        if table.size < ours {
            return Err(AbiError::Short {
                engine: table.size,
                server: ours,
            });
        }
        if table.core.is_null() {
            return Err(AbiError::NoCore);
        }
        Ok(Api(table))
    }

    pub fn table(&self) -> &'static K7EngineApi {
        self.0
    }

    pub fn core(&self) -> &'static K7CoreApi {
        unsafe { &*self.0.core }
    }

    /// False when ProjectDoc has another layout in another object: a half-rebuilt engine.
    pub fn layout_consistent(&self) -> bool {
        unsafe { (self.core().layout_consistent)() != 0 }
    }

    pub fn install_crash_handler(&self) {
        unsafe { (self.core().install_crash_handler)() }
    }

    /// The `--settings-dir` value, empty for none; `$K7_SETTINGS_DIR` is read as well.
    pub fn init_settings_dir(&self, flag: &str) {
        unsafe { (self.core().init_settings_dir)(K7Str::of(flag)) }
    }

    pub fn settings_dir(&self) -> String {
        ffi::collect_one(|ctx, out| unsafe { (self.core().settings_dir)(ctx, out) })
    }

    pub fn state_path(&self) -> String {
        ffi::collect_one(|ctx, out| unsafe { (self.core().state_path)(ctx, out) })
    }

    pub fn install_terminate_handler(&self) {
        unsafe { (self.core().install_terminate_handler)() }
    }

    /// After `enable_load_guards`: the file the last engine died loading, empty when it died elsewhere.
    pub fn previous_load_death(&self) -> String {
        ffi::collect_one(|ctx, out| unsafe { (self.core().previous_load_death)(ctx, out) })
    }

    /// After `enable_load_guards`: the external plugin the last engine started loading last, empty for none.
    pub fn previous_last_plugin(&self) -> String {
        ffi::collect_one(|ctx, out| unsafe { (self.core().previous_last_plugin)(ctx, out) })
    }

    pub fn enable_load_guards(&self) {
        unsafe { (self.core().enable_load_guards)() }
    }

    pub fn set_safe_mode(&self, on: bool) {
        unsafe { (self.core().set_safe_mode)(i32::from(on)) }
    }

    pub fn use_default_path_roots(&self) {
        unsafe { (self.core().use_default_path_roots)() }
    }

    /// Set before the server accepts, cleared with `None` after it stops; a sink set is never freed, being one per run.
    pub fn set_warning_sink(&self, sink: Option<WarningSink>) {
        match sink {
            None => unsafe { (self.core().set_warning_sink)(None, std::ptr::null_mut()) },
            Some(sink) => {
                let ctx: &'static mut WarningSink = Box::leak(Box::new(sink));
                let ctx = (ctx as *mut WarningSink).cast::<c_void>();
                unsafe { (self.core().set_warning_sink)(Some(warning_trampoline), ctx) }
            }
        }
    }

    /// The host, on the thread that will run it: the first JUCE call makes that thread the message thread.
    pub fn create_host(&self) -> Option<Host> {
        let ptr = unsafe { (self.core().host_create)() };
        if ptr.is_null() {
            return None;
        }
        let inner = Inner {
            api: *self,
            host: RwLock::new(Some(HostPtr(ptr))),
        };
        Some(Host {
            engine: Engine {
                inner: Arc::new(inner),
            },
            _not_send: PhantomData,
        })
    }
}

unsafe extern "C" fn warning_trampoline(ctx: *mut c_void, w: *const K7Warning) {
    let sink = unsafe { &*ctx.cast::<WarningSink>() };
    let w = unsafe { &*w };
    let warning = unsafe {
        Warning {
            session: w.session.to_string_lossy(),
            subject: w.subject.to_string_lossy(),
            message: w.message.to_string_lossy(),
        }
    };
    let _ = catch_unwind(AssertUnwindSafe(|| sink(warning)));
}

#[derive(Clone, Copy)]
struct HostPtr(*mut K7Host);

// The pointer is only dereferenced by the engine, whose any-thread functions lock what they read.
unsafe impl Send for HostPtr {}
unsafe impl Sync for HostPtr {}

struct Inner {
    api: Api,
    host: RwLock<Option<HostPtr>>,
}

/// The host's owner, on its message thread: it runs the loop, and destroying it ends every `Engine`'s reach.
pub struct Host {
    engine: Engine,
    _not_send: PhantomData<*const ()>,
}

impl Host {
    pub fn engine(&self) -> Engine {
        self.engine.clone()
    }

    /// Blocks on the message loop until a quit.
    pub fn run(&self) {
        if let Some(HostPtr(ptr)) = self.engine.ptr() {
            unsafe { (self.engine.inner.api.core().run)(ptr) }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let taken = self
            .engine
            .inner
            .host
            .write()
            .map(|mut h| h.take())
            .unwrap_or(None);
        if let Some(HostPtr(ptr)) = taken {
            unsafe { (self.engine.inner.api.core().host_destroy)(ptr) }
        }
    }
}

/// The handle every task holds: posts to the message thread, and the answers any thread may read.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

/// One posted job, boxed whole so its pointer crosses the C ABI thin.
struct Job {
    host: *mut K7Host,
    api: Api,
    run: Box<dyn FnOnce(Mt<'_>) + Send>,
}

unsafe extern "C" fn run_job(ctx: *mut c_void) {
    let job = unsafe { Box::from_raw(ctx.cast::<Job>()) };
    let Job { host, api, run } = *job;
    let mt = Mt {
        host,
        api,
        _on_thread: PhantomData,
    };
    if catch_unwind(AssertUnwindSafe(|| run(mt))).is_err() {
        say!("[server] a job on the message thread panicked");
    }
}

unsafe extern "C" fn drop_job(ctx: *mut c_void) {
    let job = unsafe { Box::from_raw(ctx.cast::<Job>()) };
    let _ = catch_unwind(AssertUnwindSafe(move || drop(job)));
}

impl Engine {
    pub fn api(&self) -> Api {
        self.inner.api
    }

    fn ptr(&self) -> Option<HostPtr> {
        self.inner.host.read().ok().and_then(|h| *h)
    }

    /// The host's pointer while it lives, for an area module or the fake engine's inspection.
    pub fn raw(&self) -> Option<*mut K7Host> {
        self.ptr().map(|HostPtr(p)| p)
    }

    /// Queues `job` for the message thread; false, and `job` dropped, once the host is gone.
    pub fn post(&self, job: impl FnOnce(Mt<'_>) + Send + 'static) -> bool {
        let Ok(guard) = self.inner.host.read() else {
            return false;
        };
        let Some(HostPtr(host)) = *guard else {
            return false;
        };
        let ctx = Box::into_raw(Box::new(Job {
            host,
            api: self.inner.api,
            run: Box::new(job),
        }));
        unsafe { (self.inner.api.core().post)(host, Some(run_job), Some(drop_job), ctx.cast()) };
        true
    }

    /// `job`'s answer from the message thread, or `None` when it never ran.
    pub async fn call<R: Send + 'static>(
        &self,
        job: impl FnOnce(Mt<'_>) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if !self.post(move |mt| {
            let _ = tx.send(job(mt));
        }) {
            return None;
        }
        rx.await.ok()
    }

    /// Any thread: the loop returns once the job running now ends.
    pub fn quit(&self) {
        if let Ok(guard) = self.inner.host.read()
            && let Some(HostPtr(host)) = *guard
        {
            unsafe { (self.inner.api.core().quit)(host) }
        }
    }

    pub fn audio_timing(&self) -> Option<AudioTiming> {
        let guard = self.inner.host.read().ok()?;
        let HostPtr(host) = (*guard)?;
        Some(unsafe { (self.inner.api.core().audio_timing)(host) })
    }

    /// How long the message thread has been silent: what the supervisor restarts a wedged engine on.
    pub fn message_thread_age_ms(&self) -> Option<f64> {
        let guard = self.inner.host.read().ok()?;
        let HostPtr(host) = (*guard)?;
        Some(unsafe { (self.inner.api.core().message_thread_age_ms)(host) })
    }

    /// The sessions that exist, in id order.
    pub fn session_ids(&self) -> Vec<String> {
        let Ok(guard) = self.inner.host.read() else {
            return Vec::new();
        };
        let Some(HostPtr(host)) = *guard else {
            return Vec::new();
        };
        ffi::collect(|ctx, each| unsafe { (self.inner.api.core().session_ids)(host, ctx, each) })
    }

    pub fn has_session(&self, id: &str) -> bool {
        let Ok(guard) = self.inner.host.read() else {
            return false;
        };
        let Some(HostPtr(host)) = *guard else {
            return false;
        };
        unsafe { (self.inner.api.core().has_session)(host, K7Str::of(id)) != 0 }
    }
}

/// What a job on the message thread is handed: the calls that may only run there.
pub struct Mt<'a> {
    host: *mut K7Host,
    api: Api,
    _on_thread: PhantomData<&'a ()>,
}

impl Mt<'_> {
    pub fn api(&self) -> Api {
        self.api
    }

    /// The host's pointer, for an area module's calls.
    pub fn host(&self) -> *mut K7Host {
        self.host
    }

    pub fn open_audio_device(&self) {
        unsafe { (self.api.core().open_audio_device)(self.host) }
    }

    /// The open output's name, empty when none is open.
    pub fn output_device_name(&self) -> String {
        ffi::collect_one(|ctx, out| unsafe {
            (self.api.core().output_device_name)(self.host, ctx, out)
        })
    }

    pub fn suspend_audio(&self, suspended: bool) {
        unsafe { (self.api.core().suspend_audio)(self.host, i32::from(suspended)) }
    }
}
