// scaffold_set.rs — a new SET: two decks meeting at the mixer, an empty set list and the desk that plays them, as plain files.
// A reader would get wrong that the text is elsewhere: only the project's name is put in here, the rest is `scaffold_set_*.rs`.

use crate::error::Res;
use crate::fs::Fs;
use crate::scaffold::{quote, write_project_files};
use crate::scaffold_set_files::SET_FILES;
use crate::scaffold_set_nodes::SET_NODES;
use crate::scaffold_set_sources::SET_SOURCES;

/// What the generator wrote where the name goes, quoted as the file spells a string.
const NAME: &str = "\"@@NAME@@\"";

/// The files of a minimal set, keyed by path relative to the project folder.
#[must_use]
pub fn set_project_files(name: &str) -> Vec<(String, String)> {
    let named = quote(name);
    [SET_FILES, SET_NODES, SET_SOURCES]
        .into_iter()
        .flatten()
        .map(|(k, t)| ((*k).to_owned(), t.replace(NAME, &named)))
        .collect()
}

/// Write a new set under `dir`; answers the project-relative paths written.
///
/// # Errors
/// A file that does not read, or the file system's.
pub fn scaffold_set(fs: &mut dyn Fs, dir: &str, name: &str) -> Res<Vec<String>> {
    write_project_files(fs, dir, &set_project_files(name))
}
