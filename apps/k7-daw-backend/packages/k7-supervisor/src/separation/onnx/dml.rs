//! dml.rs — what only Windows does to a session: DirectML as the provider, and DirectML.dll's folder searched when
//! the library loads. The adapter is chosen by DXGI (`win::dml_choice`): a discrete GPU by its index, and an
//! integrated one never asked, since it shares the CPU's memory and only slows the machine.

use std::path::Path;

use ort::ep::DirectML;
use ort::ep::directml::{DeviceFilter, PerformancePreference};
use ort::session::builder::SessionBuilder;

use crate::win::dml_choice::{Choice, choose};
use crate::win::dxgi;

/// Loads `lib` with its own folder searched first, so the DirectML.dll beside it is the one it gets; held until exit.
pub fn preload(lib: &Path) {
    use libloading::os::windows::{
        LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library,
    };
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;
    if let Ok(held) = unsafe { Library::load_with_flags(lib, flags) } {
        std::mem::forget(held);
    }
}

/// DirectML appended, with the memory pattern off and sequential execution it requires.
pub fn append(b: SessionBuilder) -> Result<(SessionBuilder, String), String> {
    let (provider, adapter) = match choose(&dxgi::adapters()) {
        Choice::IntegratedOnly { name } => {
            return Err(format!(
                "{name} is integrated, so this machine has no GPU for it"
            ));
        }
        Choice::Discrete { device_id, name } => (
            DirectML::default().with_device_id(device_id as i32),
            format!(" ({name}, deviceId {device_id})"),
        ),
        // DXGI listed nothing: ONNX Runtime's own filter asks for a GPU and prefers the fast one.
        Choice::Unknown => (
            DirectML::default()
                .with_device_filter(DeviceFilter::Gpu)
                .with_performance_preference(PerformancePreference::HighPerformance),
            String::new(),
        ),
    };
    let provider = provider.build().error_on_failure();
    let b = b
        .with_memory_pattern(false)
        .map_err(|e| e.to_string())?
        .with_parallel_execution(false)
        .map_err(|e| e.to_string())?
        .with_execution_providers([provider])
        .map_err(|e| e.to_string())?;
    Ok((b, adapter))
}
