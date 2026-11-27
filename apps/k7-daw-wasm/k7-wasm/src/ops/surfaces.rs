// ops/surfaces.rs — the surfaces.* operations: the Surfaces tab's rows, the panes remotes hold, what a History row lights, the mastering strip's modules.
// A reader would get wrong that the rules are in the `chains` crate: this file is the wire (names and shapes) over them.

use crate::error::WasmError;
use crate::ops::write;
use chains::bindings::SurfaceBinding;
use chains::history_marks::{MarkGraph, history_marks};
use chains::master_controls::mapped_modules;
use chains::pane_in_use::{MainSurfaceIds, RemoteScreen, in_use_by, next_free, same_screens};
use chains::surface_rows::{SurfaceRowsArgs, surface_groups};
use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InUseArgs {
    pub screens: Vec<RemoteScreen>,
    pub ids: MainSurfaceIds,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NextFreeArgs {
    pub order: Vec<String>,
    pub from: String,
    pub blocked: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SameScreensArgs {
    pub a: Vec<RemoteScreen>,
    pub b: Vec<RemoteScreen>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct HistoryArgs {
    pub paths: Vec<String>,
    pub label: Option<String>,
    pub graph: Option<MarkGraph>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct MasterModulesArgs {
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<std::collections::HashMap<String, Option<SurfaceBinding>>>")
    )]
    pub mapping: Option<IndexMap<String, Option<SurfaceBinding>>>,
}

/// The surfaces family's requests: `{"op": "surfaces.rows", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum SurfacesRequest {
    #[serde(rename = "surfaces.rows")]
    Rows(SurfaceRowsArgs),
    #[serde(rename = "surfaces.inUse")]
    InUse(InUseArgs),
    #[serde(rename = "surfaces.nextFree")]
    NextFree(NextFreeArgs),
    #[serde(rename = "surfaces.sameScreens")]
    SameScreens(SameScreensArgs),
    #[serde(rename = "surfaces.history")]
    History(HistoryArgs),
    #[serde(rename = "surfaces.masterModules")]
    MasterModules(MasterModulesArgs),
}

pub fn dispatch(request: SurfacesRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use SurfacesRequest as R;
    match request {
        R::Rows(a) => write(out, &surface_groups(&a)),
        R::InUse(a) => write(out, &in_use_by(&a.screens, &a.ids)),
        R::NextFree(a) => write(out, &next_free(&a.order, &a.from, &a.blocked)),
        R::SameScreens(a) => write(out, &same_screens(&a.a, &a.b)),
        R::History(a) => write(
            out,
            &history_marks(&a.paths, a.label.as_deref().unwrap_or(""), a.graph.as_ref()),
        ),
        R::MasterModules(a) => write(out, &mapped_modules(&a.mapping.unwrap_or_default())),
    }
}
