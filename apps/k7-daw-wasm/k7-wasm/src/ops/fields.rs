// ops/fields.rs — the fields.* operations: the editor's completions for the file at a path, by its key path.
// A reader would get wrong that a path no kind claims answers an empty list, never an error: the editor asks on every keystroke.

use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song::views::fields::{fields_at, file_kind_for, values_at};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KindArgs {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AtArgs {
    pub path: String,
    /// The editor's key path: map keys, with list items skipped; empty at the top level.
    pub at: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValuesArgs {
    pub path: String,
    pub at: Vec<String>,
    pub field: String,
}

/// The fields family's requests: `{"op": "fields.at", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum FieldsRequest {
    #[serde(rename = "fields.kind")]
    Kind(KindArgs),
    #[serde(rename = "fields.at")]
    At(AtArgs),
    #[serde(rename = "fields.values")]
    Values(ValuesArgs),
}

fn borrowed(at: &[String]) -> Vec<&str> {
    at.iter().map(String::as_str).collect()
}

pub fn dispatch(request: FieldsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        FieldsRequest::Kind(a) => write(out, &file_kind_for(&a.path)),
        FieldsRequest::At(a) => {
            let names = file_kind_for(&a.path).map_or_else(Vec::new, |k| fields_at(k, &borrowed(&a.at)));
            write(out, &names)
        }
        FieldsRequest::Values(a) => {
            let names = file_kind_for(&a.path).map_or_else(Vec::new, |k| values_at(k, &borrowed(&a.at), &a.field));
            write(out, &names)
        }
    }
}
