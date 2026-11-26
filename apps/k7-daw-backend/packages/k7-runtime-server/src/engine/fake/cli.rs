//! cli.rs — the fake engine's cli area: every mode answers 0 and logs what it was handed, so a test of argv reads
//! which mode ran with which arguments and documents.
//! What a reader gets wrong: the log is process-wide, as the modes have no host; a test reads it by a name it chose.

use super::super::cli::K7CliApi;
use super::super::ffi::K7Str;
use std::sync::Mutex;

static ASKED: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());

/// Every call so far: the mode's name, then its arguments as text.
pub fn asked() -> Vec<Vec<String>> {
    ASKED.lock().map(|a| a.clone()).unwrap_or_default()
}

fn log(call: Vec<String>) -> i32 {
    if let Ok(mut a) = ASKED.lock() {
        a.push(call);
    }
    0
}

fn s(v: K7Str) -> String {
    unsafe { v.to_string_lossy() }
}

unsafe extern "C" fn scan_plugin_child(format: K7Str, id: K7Str) -> i32 {
    log(vec!["scan-child".into(), s(format), s(id)])
}

unsafe extern "C" fn scan_plugins(force: i32) -> i32 {
    log(vec!["scan".into(), force.to_string()])
}

unsafe extern "C" fn dump_params(ty: K7Str) -> i32 {
    log(vec!["dump-params".into(), s(ty)])
}

unsafe extern "C" fn dump_params_ext(format: K7Str, id: K7Str, state: K7Str) -> i32 {
    log(vec!["dump-params-ext".into(), s(format), s(id), s(state)])
}

unsafe extern "C" fn dump_builtin_params() -> i32 {
    log(vec!["dump-builtin-params".into()])
}

unsafe extern "C" fn dump_surge_layout() -> i32 {
    log(vec!["dump-surge-layout".into()])
}

unsafe extern "C" fn list_audio_devices() -> i32 {
    log(vec!["list-audio-devices".into()])
}

unsafe extern "C" fn multi_edit_test(
    docs: *const K7Str,
    count: usize,
    seconds: f64,
    device: K7Str,
    device_type: K7Str,
) -> i32 {
    let docs = unsafe { std::slice::from_raw_parts(docs, count) };
    let mut call = vec![
        "multi-edit".into(),
        seconds.to_string(),
        s(device),
        s(device_type),
    ];
    call.extend(docs.iter().map(|d| s(*d)));
    log(call)
}

unsafe extern "C" fn render_offline(doc: K7Str, base: K7Str, out: K7Str, stems: i32) -> i32 {
    log(vec![
        "render".into(),
        s(doc),
        s(base),
        s(out),
        stems.to_string(),
    ])
}

unsafe extern "C" fn decode(path: K7Str) -> i32 {
    log(vec!["decode".into(), s(path)])
}

pub static TABLE: K7CliApi = K7CliApi {
    scan_plugin_child,
    scan_plugins,
    dump_params,
    dump_params_ext,
    dump_builtin_params,
    dump_surge_layout,
    list_audio_devices,
    multi_edit_test,
    render_offline,
    decode,
};
