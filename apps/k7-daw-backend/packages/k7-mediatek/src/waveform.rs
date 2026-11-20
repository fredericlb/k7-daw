//! waveform.rs — `apps/k7-daw-frontend/packages/audio-analysis`'s WebAssembly module, the very one TypeScript runs, in Wasmtime: compiled
//! once per front and shared (`Shared`), an instance per file whose host calls read and write `Files` (`transcode.rs`, `analyse.rs`).
//! What a reader gets wrong: it is not the Rust crate linked natively, whose SIMD kernels and libm write other bytes.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use wasmtime::{Caller, Config, Engine, Extern, Instance, Linker, Memory, Module, Store};

/// The module, as `build.rs` decoded it from the committed TypeScript.
const WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/waveform.wasm"));

/// The files one instance reads and writes, as transcode.ts's `Session` holds them.
#[derive(Default)]
pub(crate) struct Files {
    pub(crate) input: Option<File>,
    pub(crate) output: Option<File>,
}

/// The compiled module, shared by every instance of a process.
pub struct Waveform {
    engine: Engine,
    module: Module,
}

fn memory_of(caller: &mut Caller<'_, Files>) -> Option<Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(m)) => Some(m),
        _ => None,
    }
}

/// `read(ptr, len, pos)`: the input's bytes at `pos` into the module's memory; the count, or -1.
pub(crate) fn host_read(mut caller: Caller<'_, Files>, ptr: i32, len: i32, pos: f64) -> i32 {
    let Some(memory) = memory_of(&mut caller) else {
        return -1;
    };
    let (data, files) = memory.data_and_store_mut(&mut caller);
    let (Some(file), Ok(at), Ok(n)) = (
        files.input.as_ref(),
        usize::try_from(ptr),
        usize::try_from(len),
    ) else {
        return -1;
    };
    let Some(buf) = data.get_mut(at..at.saturating_add(n)) else {
        return -1;
    };
    read_at(file, buf, pos as u64).map_or(-1, |n| n as i32)
}

/// `write(ptr, len, pos)`: the module's bytes written to the output at `pos`; the count, or -1.
pub(crate) fn host_write(mut caller: Caller<'_, Files>, ptr: i32, len: i32, pos: f64) -> i32 {
    let Some(memory) = memory_of(&mut caller) else {
        return -1;
    };
    let (data, files) = memory.data_and_store_mut(&mut caller);
    let (Some(file), Ok(at), Ok(n)) = (
        files.output.as_ref(),
        usize::try_from(ptr),
        usize::try_from(len),
    ) else {
        return -1;
    };
    let Some(buf) = data.get(at..at.saturating_add(n)) else {
        return -1;
    };
    write_at(file, buf, pos as u64).map_or(-1, |()| n as i32)
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], pos: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, pos)
}

#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], pos: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, pos)
}

#[cfg(unix)]
fn write_at(file: &File, buf: &[u8], pos: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, pos)
}

#[cfg(windows)]
fn write_at(file: &File, mut buf: &[u8], mut pos: u64) -> std::io::Result<()> {
    while !buf.is_empty() {
        let n = std::os::windows::fs::FileExt::seek_write(file, buf, pos)?;
        if n == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        buf = &buf[n..];
        pos += n as u64;
    }
    Ok(())
}

/// The engine: Wasmtime's defaults hold every feature the Rust wasm32 target emits.
fn engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    // Where a host may not make memory executable (iOS), modules run as Pulley bytecode with no signal handler.
    if cfg!(target_os = "ios") {
        config.target("pulley64")?;
        config.signals_based_traps(false);
        config.memory_reservation(0);
        config.memory_guard_size(0);
    }
    Engine::new(&config)
}

impl Waveform {
    /// The module compiled, or loaded from `cache` where an earlier start kept it (the process's own folder).
    pub fn new(cache: Option<&Path>) -> Result<Waveform, String> {
        let engine = engine().map_err(|e| e.to_string())?;
        let module = match cache {
            Some(dir) => cached(&engine, dir),
            None => Module::new(&engine, WASM),
        }
        .map_err(|e| e.to_string())?;
        Ok(Waveform { engine, module })
    }

    /// A fresh store and instance of the module, its host calls answered from `Files`: one per file.
    pub(crate) fn instance(&self) -> Result<(Store<Files>, Instance), String> {
        let mut store = Store::new(&self.engine, Files::default());
        let mut linker = Linker::new(&self.engine);
        let e = |e: wasmtime::Error| e.to_string();
        linker.func_wrap("host", "read", host_read).map_err(e)?;
        linker.func_wrap("host", "write", host_write).map_err(e)?;
        let instance = linker.instantiate(&mut store, &self.module).map_err(e)?;
        Ok((store, instance))
    }
}

/// The module of one front, compiled on first use: the file service holds the one.
pub struct Shared {
    cache: Option<PathBuf>,
    module: OnceLock<Result<Arc<Waveform>, String>>,
}

impl Shared {
    /// Compiled modules are kept in `cache`, the front's own folder, between starts.
    pub fn new(cache: Option<PathBuf>) -> Arc<Shared> {
        Arc::new(Shared {
            cache,
            module: OnceLock::new(),
        })
    }

    /// The module, or why it could not be compiled; a failure is kept, as a host lacking it lacks it for good.
    pub fn get(&self) -> Result<Arc<Waveform>, String> {
        self.module
            .get_or_init(|| Waveform::new(self.cache.as_deref()).map(Arc::new))
            .clone()
    }
}

/// The module compiled once per machine and Wasmtime build, kept in `dir`; a stale or foreign file is compiled again.
fn cached(engine: &Engine, dir: &Path) -> wasmtime::Result<Module> {
    use sha2::{Digest, Sha256};
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    engine.precompile_compatibility_hash().hash(&mut h);
    let digest = Sha256::digest(WASM);
    let hex: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    let file = dir.join(format!("waveform-{hex}-{:016x}.cwasm", h.finish()));
    if file.is_file() {
        // SAFETY: the file is one this process serialized into its own folder; a foreign one fails the engine's check.
        if let Ok(module) = unsafe { Module::deserialize_file(engine, &file) } {
            return Ok(module);
        }
    }
    let module = Module::new(engine, WASM)?;
    // Best effort: a cache that cannot be written costs the next start a compile, nothing else.
    if let Ok(bytes) = module.serialize() {
        let _ = std::fs::create_dir_all(dir);
        let tmp = file.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &file).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    Ok(module)
}
