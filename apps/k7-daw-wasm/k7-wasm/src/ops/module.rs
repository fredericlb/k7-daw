// ops/module.rs — the module.* operations: how a module is keyed and referenced, and which files a file param can take.
// A reader would get wrong that the vocabularies (`FILE_PARAMS`, `LIBRARY_FILE_FAMILIES`) are not here: they cross once, as `consts.generated.ts`.

use crate::error::WasmError;
use crate::ops::records::PathArgs;
use crate::ops::write;
use serde::Deserialize;
use song::modules::keys::{assets_for_param, library_ref, module_key_from_path, shader_uniform_for};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LibraryRefArgs {
    pub library: String,
    pub rest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParamArgs {
    pub param: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AssetsArgs {
    pub files: Vec<String>,
    pub param: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub family: Option<String>,
}

/// The module family's requests: `{"op": "module.key_from_path", "args": {"path": "modules/audio/a/module.k7x"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum ModuleRequest {
    #[serde(rename = "module.key_from_path")]
    KeyFromPath(PathArgs),
    #[serde(rename = "module.library_ref")]
    LibraryRef(LibraryRefArgs),
    #[serde(rename = "module.shader_uniform")]
    ShaderUniform(ParamArgs),
    #[serde(rename = "module.assets_for_param")]
    AssetsForParam(AssetsArgs),
}

pub fn dispatch(request: ModuleRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        ModuleRequest::KeyFromPath(a) => write(out, &module_key_from_path(&a.path)),
        ModuleRequest::LibraryRef(a) => write(out, &library_ref(&a.library, &a.rest)),
        ModuleRequest::ShaderUniform(a) => write(out, &shader_uniform_for(&a.param)),
        ModuleRequest::AssetsForParam(a) => write(out, &assets_for_param(&a.files, &a.param, a.family.as_deref())),
    }
}
