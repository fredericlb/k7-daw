// modules/files.rs — the params whose value is a FILE, the library folders such a file may come from, and what reaches the engine.
// A reader would get wrong that `ir` and `model` are not alike: `model` is also the module's own `assets/`, `ir` is a library's alone.

use super::types::{DSP_TYPE, FAUST_TYPE, JSDSP_TYPE, KNOWN_PLUGIN_TYPES};

/// The params whose value is a FILE the plugin loads, from the module's own `assets/` or the model cache.
pub const FILE_PARAMS: [&str; 2] = ["model", "onnx"];

/// File params whose file is in a LIBRARY's shared folder, named `@<library>/<path>`.
pub const LIBRARY_FILE_PARAMS: [(&str, &str); 1] = [("ir", "irs")];

/// File params a module may fill from its own `assets/` OR from a library's shared folder.
pub const SHARED_FILE_PARAMS: [(&str, &str); 1] = [("model", "models")];

/// What a rack tap's sample picker asks for: a library's `samples/`.
pub const TAP_SAMPLE: &str = "sample";

/// The level a project's own sounds answer to, as a library's answer to its name.
pub const PROJECT_SOUNDS: &str = "project";

const AUDIO_EXTS: [&str; 4] = [".wav", ".aif", ".aiff", ".flac"];

/// The library folders that hold FILES rather than modules, and what each one is walked for.
pub const LIBRARY_FILE_FAMILIES: [(&str, &[&str]); 4] = [
    ("samples", &AUDIO_EXTS),
    ("irs", &AUDIO_EXTS),
    ("wavetables", &AUDIO_EXTS),
    ("models", &[".nam", ".json"]),
];

/// The plugin formats loaded from disk through the scan cache; the engine's `isExternalFormat`.
pub fn is_external_format(t: &str) -> bool {
    t != "internal" && KNOWN_PLUGIN_TYPES.contains(&t)
}

/// The library folder a file param can ALSO name a file in, by `@<library>/<path>`.
pub fn shared_family_of(param: &str) -> Option<&'static str> {
    SHARED_FILE_PARAMS.iter().find(|(p, _)| *p == param).map(|(_, f)| *f)
}

/// The folder an `ir`-like param's file lives in.
pub fn library_family_of(param: &str) -> Option<&'static str> {
    LIBRARY_FILE_PARAMS.iter().find(|(p, _)| *p == param).map(|(_, f)| *f)
}

/// Whether `path` is a file a library's `family` folder holds.
pub fn is_library_file_of(family: &str, path: &str) -> bool {
    let lower = path.to_lowercase();
    LIBRARY_FILE_FAMILIES
        .iter()
        .find(|(f, _)| *f == family)
        .is_some_and(|(_, exts)| exts.iter().any(|e| lower.ends_with(e)))
}

/// Whether a module's `plugin.type` becomes an engine plugin; `offline` does not, its pass writes to disk first.
pub fn reaches_engine(raw_type: &str) -> bool {
    let t = raw_type.to_lowercase();
    t == "internal" || t == JSDSP_TYPE || t == DSP_TYPE || t == FAUST_TYPE || is_external_format(&t)
}
