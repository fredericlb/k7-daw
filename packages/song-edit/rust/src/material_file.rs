// material_file.rs — READ the window a material file states; nothing writes one, the window lives on the event.
// A reader would get wrong the kinds: an edge is a number (seconds into a source) or a string (a position), nothing else.

use crate::error::Res;
use crate::fs::Fs;
use crate::workspace_doc::ws_data;
use k7x::edit::Value;

/// The window a material file already declares, as authored (absent stays absent).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialEdges {
    pub from: Option<Value>,
    pub to: Option<Value>,
}

/// The `from` and `to` of a material file when each is a number or a string.
///
/// # Errors
/// A file that does not read.
pub fn material_window_of(fs: &dyn Fs, file: &str) -> Res<MaterialEdges> {
    let data = ws_data(fs, file)?;
    let one = |k: &str| {
        data.get(k)
            .filter(|v| matches!(v, Value::Num(_) | Value::Str(_)))
            .cloned()
    };
    Ok(MaterialEdges {
        from: one("from"),
        to: one("to"),
    })
}
