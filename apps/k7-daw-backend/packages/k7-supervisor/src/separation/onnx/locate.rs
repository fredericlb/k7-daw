//! locate.rs — where ONNX Runtime's library is looked for, and which file in a folder it is: `$STUDIO_ONNXRUNTIME_DIR`
//! first, then the places a build or a package puts it relative to this executable. Pure paths, no loading.

use std::path::{Path, PathBuf};

/// Where a build or a package puts the library, `$STUDIO_ONNXRUNTIME_DIR` first, then relative to `exe`.
pub fn candidate_dirs(env_dir: Option<&str>, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = env_dir.filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d));
    }
    if let Some(d) = exe.and_then(Path::parent) {
        dirs.push(d.to_path_buf());
        dirs.push(d.join("onnxruntime"));
        dirs.push(d.join("resources").join("onnxruntime"));
        dirs.push(
            d.join("..")
                .join("Resources")
                .join("resources")
                .join("onnxruntime"),
        );
    }
    dirs
}

/// This process's candidates.
pub fn candidates() -> Vec<PathBuf> {
    let env_dir = std::env::var("STUDIO_ONNXRUNTIME_DIR").ok();
    candidate_dirs(env_dir.as_deref(), std::env::current_exe().ok().as_deref())
}

/// The library file in `dir`, whatever version suffix it carries.
pub fn library_in(dir: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let file = dir.join("onnxruntime.dll");
        return file.is_file().then_some(file);
    }
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if cfg!(target_vendor = "apple") {
                name.starts_with("libonnxruntime") && name.ends_with(".dylib") && name.len() > 6
            } else {
                name.starts_with("libonnxruntime.so")
            }
        })
        .map(|e| e.path())
        .collect();
    // A folder order is whatever the disk says; the same file every run.
    names.sort();
    names.into_iter().next()
}
