//! real.rs — the session over the `ort` crate. A run can be terminated from another thread, which is how a job's
//! hour ends a provider that hangs. A provider that fails to register is an ERROR here (`error_on_failure`), never
//! a silent fall back to the CPU the caller did not ask for.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use ort::environment::Environment;
use ort::session::{RunOptions, Session};
use ort::value::TensorRef;

use super::locate;
use crate::separation::audio::{CHANNELS, SEGMENT};
use crate::separation::budget::thread_count;

/// The process's one environment, or why there is none: ORT allows one per process.
fn environment() -> &'static Result<Environment, String> {
    static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENV.get_or_init(init)
}

/// Loaded from the first folder that holds the library, so a missing one is a sentence and never a failed start.
#[cfg(feature = "dynamic-ort")]
fn init() -> Result<Environment, String> {
    let mut error = String::new();
    for dir in locate::candidates() {
        let Some(lib) = locate::library_in(&dir) else {
            continue;
        };
        #[cfg(windows)]
        super::dml::preload(&lib);
        match ort::init_from(&lib) {
            Ok(b) => match b.with_name("k7-supervisor").with_telemetry(false).build() {
                Ok(env) => return Ok(env),
                Err(e) => error = format!("{} is not a usable ONNX Runtime ({e})", lib.display()),
            },
            Err(ort::LoadDynamicError::Dlopen { error: e, .. }) => {
                error = format!("could not load {}: {e}", lib.display());
            }
            Err(e) => error = format!("{} is not a usable ONNX Runtime ({e})", lib.display()),
        }
    }
    if error.is_empty() {
        error = "ONNX Runtime is not installed beside the supervisor".into();
    }
    Err(error)
}

/// Linked at build time (iOS: the package's static library), so there is nothing to look for.
#[cfg(not(feature = "dynamic-ort"))]
fn init() -> Result<Environment, String> {
    ort::init()
        .with_name("k7-supervisor")
        .build()
        .map_err(|e| e.to_string())
}

pub fn available() -> bool {
    environment().is_ok()
}

/// A cut's CPU threads (`thread_count`): `$STUDIO_ONNX_THREADS` sets them outright.
fn intra_op_threads() -> usize {
    let set = std::env::var("STUDIO_ONNX_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| (1..=256).contains(n));
    set.unwrap_or_else(|| {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        thread_count(cores, cfg!(target_os = "ios"))
    })
}

pub struct OnnxSession {
    session: Mutex<Session>,
    run_options: Arc<RunOptions>,
    provider: String,
    adapter: String,
    skipped: String,
    output: String,
}

impl OnnxSession {
    /// Open `model` on the first of `providers` that loads; fails naming each failure.
    pub fn open(model: &Path, providers: &[String]) -> Result<OnnxSession, String> {
        let env = environment().as_ref().map_err(Clone::clone)?;
        let mut failures = String::new();
        for ep in providers {
            match open_on(env, model, ep) {
                Ok((session, adapter)) => {
                    let output = output_name(&session);
                    let run_options = RunOptions::new().map_err(|e| e.to_string())?;
                    return Ok(OnnxSession {
                        session: Mutex::new(session),
                        run_options: Arc::new(run_options),
                        provider: ep.clone(),
                        adapter,
                        skipped: failures,
                        output,
                    });
                }
                Err(e) => {
                    let sep = if failures.is_empty() { "" } else { "; " };
                    failures = format!("{failures}{sep}{ep}: {e}");
                }
            }
        }
        Err(format!(
            "no ONNX execution provider would load {} — {failures}",
            model.display()
        ))
    }

    /// One window `[1, 2, SEGMENT]` in, the stems `[1, 4, 2, SEGMENT]` out, flat.
    pub fn run(&self, mix: &[f32]) -> Result<Vec<f32>, String> {
        let dims = [1i64, CHANNELS as i64, SEGMENT as i64];
        let input = TensorRef::from_array_view((dims, mix)).map_err(|e| e.to_string())?;
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let outputs = session
            .run_with_options(ort::inputs!["mix" => input], &*self.run_options)
            .map_err(|e| e.to_string())?;
        let (_, data) = outputs[self.output.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        Ok(data.to_vec())
    }

    /// Abort a `run` in progress on another thread; safe to call at any time.
    pub fn terminate(&self) {
        let _ = self.run_options.terminate();
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// ` (name, deviceId N)` for a named GPU, else empty.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// The providers tried before this one and why each gave way; empty when the first won.
    pub fn skipped(&self) -> &str {
        &self.skipped
    }
}

/// The first output, or the one named `stems` when the graph has it.
fn output_name(session: &Session) -> String {
    let outs = session.outputs();
    outs.iter()
        .map(|o| o.name())
        .find(|n| *n == "stems")
        .or_else(|| outs.first().map(|o| o.name()))
        .unwrap_or("stems")
        .to_string()
}

/// One session on provider `ep`, and the adapter it chose when it names one.
fn open_on(env: &Environment, model: &Path, ep: &str) -> Result<(Session, String), String> {
    let mut b = Session::builder(env)
        .map_err(|e| e.to_string())?
        .with_intra_threads(intra_op_threads())
        .map_err(|e| e.to_string())?;
    let mut adapter = String::new();
    match ep {
        "cpu" => {}
        "coreml" => b = with_coreml(b)?,
        "dml" => (b, adapter) = with_dml(b)?,
        _ => return Err("not in this build".into()),
    }
    Ok((
        b.commit_from_file(model).map_err(|e| e.to_string())?,
        adapter,
    ))
}

// CoreML stays out of a phone, which cannot compile HT-Demucs anyway.
#[cfg(all(target_vendor = "apple", not(target_os = "ios")))]
fn with_coreml(
    b: ort::session::builder::SessionBuilder,
) -> Result<ort::session::builder::SessionBuilder, String> {
    b.with_execution_providers([ort::ep::CoreML::default().build().error_on_failure()])
        .map_err(|e| e.to_string())
}

#[cfg(not(all(target_vendor = "apple", not(target_os = "ios"))))]
fn with_coreml(
    _: ort::session::builder::SessionBuilder,
) -> Result<ort::session::builder::SessionBuilder, String> {
    Err("CoreML is not in this build of ONNX Runtime".into())
}

#[cfg(windows)]
fn with_dml(
    b: ort::session::builder::SessionBuilder,
) -> Result<(ort::session::builder::SessionBuilder, String), String> {
    super::dml::append(b)
}

#[cfg(not(windows))]
fn with_dml(
    _: ort::session::builder::SessionBuilder,
) -> Result<(ort::session::builder::SessionBuilder, String), String> {
    Err("DirectML exists only on Windows".into())
}
