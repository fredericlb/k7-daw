// ops/library.rs — the library.* operations: the LIBRARIES tab's rows and the rules over a library path.
// A reader would get wrong that every list arrives already in people's order: the page's collator sorts, and the rows keep it.

use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song_edit::library_tree::{
    LibraryAssemblyRow, LibraryChainRow, LibraryLevel, file_folder_of, is_library_path, library_module_ref,
    library_of_path, library_tree, read_only_paths, root_of,
};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, rename = "LibraryTreeArgs"))]
pub struct TreeArgs {
    pub levels: Vec<LibraryLevel>,
    pub chains: Option<Vec<LibraryChainRow>>,
    pub assemblies: Option<Vec<LibraryAssemblyRow>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "LibraryLevelsArgs"))]
pub struct LevelsArgs {
    pub levels: Vec<LibraryLevel>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "LibraryRootArgs"))]
pub struct RootArgs {
    pub levels: Vec<LibraryLevel>,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "LibraryPathArgs"))]
pub struct PathArgs {
    pub path: String,
}

/// The library family's requests: `{"op": "library.tree", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum LibraryRequest {
    #[serde(rename = "library.tree")]
    Tree(TreeArgs),
    #[serde(rename = "library.readOnly")]
    ReadOnly(LevelsArgs),
    #[serde(rename = "library.rootOf")]
    RootOf(RootArgs),
    #[serde(rename = "library.fileFolder")]
    FileFolder(PathArgs),
    #[serde(rename = "library.moduleRef")]
    ModuleRef(PathArgs),
    #[serde(rename = "library.isPath")]
    IsPath(PathArgs),
    #[serde(rename = "library.ofPath")]
    OfPath(PathArgs),
}

pub fn dispatch(request: LibraryRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use LibraryRequest as R;
    match request {
        R::Tree(a) => write(
            out,
            &library_tree(
                &a.levels,
                &a.chains.unwrap_or_default(),
                &a.assemblies.unwrap_or_default(),
            ),
        ),
        R::ReadOnly(a) => write(out, &read_only_paths(&a.levels)),
        R::RootOf(a) => write(out, &root_of(&a.levels, &a.path)),
        R::FileFolder(a) => write(out, &file_folder_of(&a.path)),
        R::ModuleRef(a) => write(out, &library_module_ref(&a.path)),
        R::IsPath(a) => write(out, &is_library_path(&a.path)),
        R::OfPath(a) => write(out, &library_of_path(&a.path)),
    }
}
