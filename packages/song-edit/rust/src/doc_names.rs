// doc_names.rs — the workspace documents a folder holds, by name, the way the time and loop edits list a lane or curve folder.
// A reader would get wrong what failure means: a folder that is absent or will not list holds none, never an error.

use crate::fs::Fs;
use crate::workspace_file::{is_doc_name, ws_exists};

/// Every `*.k7x` name in `at`, in `Array.sort`'s UTF-16 order; empty when the folder is absent or does not list.
#[must_use]
pub fn docs_in(fs: &dyn Fs, at: &str) -> Vec<String> {
    if !ws_exists(fs, at) {
        return vec![];
    }
    let Ok(entries) = fs.list(at) else { return vec![] };
    let mut names: Vec<String> = entries.into_iter().map(|e| e.name).filter(|f| is_doc_name(f)).collect();
    names.sort_by_key(|n| n.encode_utf16().collect::<Vec<u16>>());
    names
}
