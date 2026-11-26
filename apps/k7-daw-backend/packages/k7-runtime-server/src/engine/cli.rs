//! cli.rs — the cli area of the C ABI (`k7_engine_cli.h`) made safe: the engine's one-shot modes, each the whole
//! run of the process, its exit code returned.
//! What a reader gets wrong: a document handed here is already validated (`crate::doc`), since the C++ reader
//! checks nothing; an engine without the area answers 1 and says so, rather than panicking.

use super::Api;
use super::ffi::{K7Str, table};
use crate::log::say;

table! {
    /// `k7_engine_cli.h`.
    K7CliApi {
        scan_plugin_child: unsafe extern "C" fn(format: K7Str, id: K7Str) -> i32,
        scan_plugins: unsafe extern "C" fn(force: i32) -> i32,
        dump_params: unsafe extern "C" fn(ty: K7Str) -> i32,
        dump_params_ext: unsafe extern "C" fn(format: K7Str, id: K7Str, state_file: K7Str) -> i32,
        dump_builtin_params: unsafe extern "C" fn() -> i32,
        dump_surge_layout: unsafe extern "C" fn() -> i32,
        list_audio_devices: unsafe extern "C" fn() -> i32,
        multi_edit_test: unsafe extern "C" fn(docs: *const K7Str, count: usize, seconds: f64, device: K7Str, device_type: K7Str) -> i32,
        render_offline: unsafe extern "C" fn(doc_json: K7Str, base_dir: K7Str, out_dir: K7Str, stems: i32) -> i32,
        decode: unsafe extern "C" fn(path: K7Str) -> i32,
    }
}

/// One mode, as argv asked it.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    ScanChild {
        format: String,
        id: String,
    },
    Scan {
        force: bool,
    },
    DumpParams {
        ty: String,
    },
    DumpParamsExt {
        format: String,
        id: String,
        state_file: String,
    },
    DumpBuiltinParams,
    DumpSurgeLayout,
    ListAudioDevices,
    MultiEdit {
        docs: Vec<String>,
        seconds: f64,
        device: String,
        device_type: String,
    },
    Render {
        doc: String,
        base: String,
        out: String,
        stems: bool,
    },
    Decode {
        path: String,
    },
}

impl Api {
    /// The cli area, `None` while the engine has not built it.
    pub fn cli(&self) -> Option<&'static K7CliApi> {
        let p = self.table().cli.cast::<K7CliApi>();
        unsafe { p.as_ref() }
    }

    /// Runs `mode` to its end on this thread; the process's exit code.
    pub fn run_mode(&self, mode: &Mode) -> i32 {
        let Some(c) = self.cli() else {
            say!("k7-runtime: this engine has no command-line modes");
            return 1;
        };
        let s = K7Str::of;
        unsafe {
            match mode {
                Mode::ScanChild { format, id } => (c.scan_plugin_child)(s(format), s(id)),
                Mode::Scan { force } => (c.scan_plugins)(i32::from(*force)),
                Mode::DumpParams { ty } => (c.dump_params)(s(ty)),
                Mode::DumpParamsExt {
                    format,
                    id,
                    state_file,
                } => (c.dump_params_ext)(s(format), s(id), s(state_file)),
                Mode::DumpBuiltinParams => (c.dump_builtin_params)(),
                Mode::DumpSurgeLayout => (c.dump_surge_layout)(),
                Mode::ListAudioDevices => (c.list_audio_devices)(),
                Mode::MultiEdit {
                    docs,
                    seconds,
                    device,
                    device_type,
                } => {
                    let views: Vec<K7Str> = docs.iter().map(|d| s(d)).collect();
                    (c.multi_edit_test)(
                        views.as_ptr(),
                        views.len(),
                        *seconds,
                        s(device),
                        s(device_type),
                    )
                }
                Mode::Render {
                    doc,
                    base,
                    out,
                    stems,
                } => (c.render_offline)(s(doc), s(base), s(out), i32::from(*stems)),
                Mode::Decode { path } => (c.decode)(s(path)),
            }
        }
    }
}
