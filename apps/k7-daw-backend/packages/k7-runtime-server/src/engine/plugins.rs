//! plugins.rs — the plugins area of the C ABI (`k7_engine_plugins.h`) made safe: the catalogue and its scan, a plugin
//! type's parameters, and the message-thread calls on instances, windows, presets and waveform taps.
//! What a reader gets wrong: the `Api` methods BLOCK (a scan or a probe loads binaries for seconds) and belong to a
//! blocking task, never a socket's task or the message thread; an engine without the area answers empty or false.

use super::ffi::{K7Host, K7Str, K7StrEach};
use super::state::{K7ParamInfoEach, ParamInfo, info_collector};
use super::{Api, Mt};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[repr(C)]
pub struct K7PluginInfo {
    pub name: K7Str,
    pub format: K7Str,
    pub manufacturer: K7Str,
    pub instrument: i32,
    pub role: K7Str,
    pub category: K7Str,
    pub id: K7Str,
}

#[repr(C)]
pub struct K7PluginInstance {
    pub id: K7Str,
    pub name: K7Str,
    pub has_native_ui: i32,
}

#[repr(C)]
pub struct K7Preset {
    pub name: K7Str,
    pub source: K7Str,
    pub index: i32,
    pub path: K7Str,
}

pub type K7PluginEach = Option<unsafe extern "C" fn(ctx: *mut c_void, info: *const K7PluginInfo)>;
pub type K7InstanceEach =
    Option<unsafe extern "C" fn(ctx: *mut c_void, instance: *const K7PluginInstance)>;
pub type K7PresetEach = Option<unsafe extern "C" fn(ctx: *mut c_void, preset: *const K7Preset)>;
pub type K7ScanStep =
    Option<unsafe extern "C" fn(ctx: *mut c_void, done: i32, total: i32, loading: K7Str)>;

/// `k7_engine_plugins.h`'s table, field for field (`tests/all/engine_abi.rs` holds the names to the header).
#[repr(C)]
pub struct K7PluginsApi {
    pub cache_warm: unsafe extern "C" fn() -> i32,
    pub load_cache: unsafe extern "C" fn(ctx: *mut c_void, each: K7PluginEach),
    pub scan: unsafe extern "C" fn(
        force: i32,
        ctx: *mut c_void,
        step: K7ScanStep,
        each: K7PluginEach,
        failed: K7StrEach,
    ) -> i32,
    pub plugin_params_of: unsafe extern "C" fn(
        format: K7Str,
        id: K7Str,
        ctx: *mut c_void,
        each: K7ParamInfoEach,
    ) -> i32,
    pub prepare_native_ui: unsafe extern "C" fn(format: K7Str, id: K7Str) -> i32,
    pub show_native_ui_of: unsafe extern "C" fn(host: *mut K7Host, format: K7Str, id: K7Str) -> i32,
    pub snapshot_orphan_to:
        unsafe extern "C" fn(host: *mut K7Host, format: K7Str, id: K7Str, path: K7Str) -> i32,
    pub show_native_ui: unsafe extern "C" fn(host: *mut K7Host, session: K7Str, id: K7Str),
    pub plugin_instances: unsafe extern "C" fn(
        host: *mut K7Host,
        session: K7Str,
        ctx: *mut c_void,
        each: K7InstanceEach,
    ),
    pub list_presets: unsafe extern "C" fn(
        host: *mut K7Host,
        session: K7Str,
        module: K7Str,
        ctx: *mut c_void,
        each: K7PresetEach,
    ),
    pub set_preset: unsafe extern "C" fn(
        host: *mut K7Host,
        session: K7Str,
        module: K7Str,
        index: i32,
        path: K7Str,
    ),
    pub set_plugin_param:
        unsafe extern "C" fn(host: *mut K7Host, session: K7Str, id: K7Str, name: K7Str, value: f64),
    pub enable_waveform_tap:
        unsafe extern "C" fn(host: *mut K7Host, session: K7Str, id: K7Str, side: K7Str),
    pub disable_waveform_tap:
        unsafe extern "C" fn(host: *mut K7Host, session: K7Str, id: K7Str, side: K7Str),
}

impl K7PluginsApi {
    pub const FIELDS: &'static [&'static str] = &[
        "cache_warm",
        "load_cache",
        "scan",
        "plugin_params_of",
        "prepare_native_ui",
        "show_native_ui_of",
        "snapshot_orphan_to",
        "show_native_ui",
        "plugin_instances",
        "list_presets",
        "set_preset",
        "set_plugin_param",
        "enable_waveform_tap",
        "disable_waveform_tap",
    ];
}

unsafe impl Sync for K7PluginsApi {}

/// One installed plugin of the catalogue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginInfo {
    pub name: String,
    pub format: String,
    pub manufacturer: String,
    pub instrument: bool,
    pub role: String,
    pub category: String,
    pub id: String,
}

/// One plugin instance of a loaded project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInstance {
    pub id: String,
    pub name: String,
    pub has_native_ui: bool,
}

/// A selectable preset: `source` is "program" (by `index`) or "file" (by `path`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub source: String,
    pub index: i32,
    pub path: String,
}

fn text(s: K7Str) -> String {
    unsafe { s.to_string_lossy() }
}

unsafe extern "C" fn plugin_collector(ctx: *mut c_void, info: *const K7PluginInfo) {
    let i = unsafe { &*info };
    let one = PluginInfo {
        name: text(i.name),
        format: text(i.format),
        manufacturer: text(i.manufacturer),
        instrument: i.instrument != 0,
        role: text(i.role),
        category: text(i.category),
        id: text(i.id),
    };
    unsafe { &mut *ctx.cast::<Vec<PluginInfo>>() }.push(one);
}

unsafe extern "C" fn instance_collector(ctx: *mut c_void, instance: *const K7PluginInstance) {
    let i = unsafe { &*instance };
    unsafe { &mut *ctx.cast::<Vec<PluginInstance>>() }.push(PluginInstance {
        id: text(i.id),
        name: text(i.name),
        has_native_ui: i.has_native_ui != 0,
    });
}

unsafe extern "C" fn preset_collector(ctx: *mut c_void, preset: *const K7Preset) {
    let p = unsafe { &*preset };
    unsafe { &mut *ctx.cast::<Vec<Preset>>() }.push(Preset {
        name: text(p.name),
        source: text(p.source),
        index: p.index,
        path: text(p.path),
    });
}

/// What a scan's three callbacks share through its one context.
struct ScanCtx<'a> {
    step: &'a mut dyn FnMut(i32, i32, &str),
    found: Vec<PluginInfo>,
    failed: String,
}

unsafe extern "C" fn scan_step(ctx: *mut c_void, done: i32, total: i32, loading: K7Str) {
    let scan = unsafe { &mut *ctx.cast::<ScanCtx<'_>>() };
    let loading = text(loading);
    let _ = catch_unwind(AssertUnwindSafe(|| (scan.step)(done, total, &loading)));
}

unsafe extern "C" fn scan_each(ctx: *mut c_void, info: *const K7PluginInfo) {
    let scan = unsafe { &mut *ctx.cast::<ScanCtx<'_>>() };
    let mut one = Vec::new();
    unsafe { plugin_collector((&mut one as *mut Vec<PluginInfo>).cast(), info) };
    scan.found.append(&mut one);
}

unsafe extern "C" fn scan_failed(ctx: *mut c_void, why: K7Str) {
    unsafe { &mut *ctx.cast::<ScanCtx<'_>>() }.failed = text(why);
}

impl Api {
    /// The plugins area, `None` when the engine has not built it.
    pub fn plugins(&self) -> Option<&'static K7PluginsApi> {
        let ptr = self.table().plugins.cast::<K7PluginsApi>();
        (!ptr.is_null()).then(|| unsafe { &*ptr })
    }

    /// Whether the catalogue's cache needs no scan.
    pub fn plugin_cache_warm(&self) -> bool {
        self.plugins()
            .is_some_and(|p| unsafe { (p.cache_warm)() } != 0)
    }

    /// The cached catalogue, built-ins first, no scanning.
    pub fn load_plugin_cache(&self) -> Vec<PluginInfo> {
        let mut out: Vec<PluginInfo> = Vec::new();
        if let Some(p) = self.plugins() {
            unsafe {
                (p.load_cache)(
                    (&mut out as *mut Vec<PluginInfo>).cast(),
                    Some(plugin_collector),
                )
            };
        }
        out
    }

    /// BLOCKING. A scan, `step` told `(done, total, loading)` as it goes; `Err` carries why it threw.
    pub fn scan_plugins(
        &self,
        force: bool,
        step: &mut dyn FnMut(i32, i32, &str),
    ) -> Result<Vec<PluginInfo>, String> {
        let Some(p) = self.plugins() else {
            return Ok(Vec::new());
        };
        let mut ctx = ScanCtx {
            step,
            found: Vec::new(),
            failed: String::new(),
        };
        let ok = unsafe {
            (p.scan)(
                i32::from(force),
                (&mut ctx as *mut ScanCtx<'_>).cast(),
                Some(scan_step),
                Some(scan_each),
                Some(scan_failed),
            )
        };
        if ok != 0 {
            Ok(ctx.found)
        } else {
            Err(ctx.failed)
        }
    }

    /// BLOCKING. A plugin type's parameters; `None` is "could not be found, created or loaded", empty is "none".
    pub fn plugin_params_of(&self, format: &str, id: &str) -> Option<Vec<ParamInfo>> {
        let p = self.plugins()?;
        let mut out: Vec<ParamInfo> = Vec::new();
        let found = unsafe {
            (p.plugin_params_of)(
                K7Str::of(format),
                K7Str::of(id),
                (&mut out as *mut Vec<ParamInfo>).cast(),
                Some(info_collector),
            )
        };
        (found != 0).then_some(out)
    }

    /// BLOCKING. Loads the plugin's orphan instance for its own window.
    pub fn prepare_native_ui(&self, format: &str, id: &str) -> bool {
        self.plugins().is_some_and(
            |p| unsafe { (p.prepare_native_ui)(K7Str::of(format), K7Str::of(id)) } != 0,
        )
    }
}

impl Mt<'_> {
    /// Opens the window of a prepared orphan.
    pub fn show_native_ui_of(&self, format: &str, id: &str) -> bool {
        self.api().plugins().is_some_and(|p| unsafe {
            (p.show_native_ui_of)(self.host(), K7Str::of(format), K7Str::of(id)) != 0
        })
    }

    /// Writes what the orphan holds to `path` as a .vstpreset.
    pub fn snapshot_orphan_to(&self, format: &str, id: &str, path: &str) -> bool {
        self.api().plugins().is_some_and(|p| unsafe {
            (p.snapshot_orphan_to)(
                self.host(),
                K7Str::of(format),
                K7Str::of(id),
                K7Str::of(path),
            ) != 0
        })
    }

    /// Opens a live plugin's own window; external plugins only.
    pub fn show_native_ui(&self, session: &str, id: &str) {
        if let Some(p) = self.api().plugins() {
            unsafe { (p.show_native_ui)(self.host(), K7Str::of(session), K7Str::of(id)) };
        }
    }

    /// The plugin instances of a session's loaded project.
    pub fn plugin_instances(&self, session: &str) -> Vec<PluginInstance> {
        let mut out: Vec<PluginInstance> = Vec::new();
        if let Some(p) = self.api().plugins() {
            unsafe {
                (p.plugin_instances)(
                    self.host(),
                    K7Str::of(session),
                    (&mut out as *mut Vec<PluginInstance>).cast(),
                    Some(instance_collector),
                )
            };
        }
        out
    }

    /// The presets a module's live external plugin offers.
    pub fn list_presets(&self, session: &str, module: &str) -> Vec<Preset> {
        let mut out: Vec<Preset> = Vec::new();
        if let Some(p) = self.api().plugins() {
            unsafe {
                (p.list_presets)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(module),
                    (&mut out as *mut Vec<Preset>).cast(),
                    Some(preset_collector),
                )
            };
        }
        out
    }

    /// A non-empty `path` wins over `index`.
    pub fn set_preset(&self, session: &str, module: &str, index: i32, path: &str) {
        if let Some(p) = self.api().plugins() {
            unsafe {
                (p.set_preset)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(module),
                    index,
                    K7Str::of(path),
                )
            };
        }
    }

    /// One automatable parameter of the live plugin, in its native range; never persisted.
    pub fn set_plugin_param(&self, session: &str, id: &str, name: &str, value: f64) {
        if let Some(p) = self.api().plugins() {
            unsafe {
                (p.set_plugin_param)(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(id),
                    K7Str::of(name),
                    value,
                )
            };
        }
    }

    pub fn waveform_tap(&self, session: &str, id: &str, side: &str, on: bool) {
        if let Some(p) = self.api().plugins() {
            let call = if on {
                p.enable_waveform_tap
            } else {
                p.disable_waveform_tap
            };
            unsafe {
                call(
                    self.host(),
                    K7Str::of(session),
                    K7Str::of(id),
                    K7Str::of(side),
                )
            };
        }
    }
}
