// routing/layout.rs — `layout.k7x`: where cards sit on the graph. Cosmetic, but a bad entry is an error and no card is placed.
// A reader would get wrong that only the root key `positions` is read: any other key is ignored, and so is a `positions` that is no object.

use super::graph_file::positions_of;
use super::hand::get;
use super::types::Position;
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexMap;
use k7x::{Bump, Node, parse_tree};

/// The file's name below the project.
pub const LAYOUT_FILE: &str = "layout.k7x";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub positions: IndexMap<String, Position>,
}

/// `layout.k7x`'s text decoded; an entry that does not read, or a syntax error, is the file's finding.
pub fn decode_layout(file: &str, text: &str) -> Result<Layout, Finding> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    let Some(e) = get(&root, "positions") else {
        return Ok(Layout::default());
    };
    if !matches!(e.value, Node::Map(_)) {
        return Ok(Layout::default());
    }
    positions_of(&e.value)
        .map(|positions| Layout { positions })
        .map_err(|f| f.finding(file, text))
}

/// The finding for a `layout.k7x` the Source refuses: the pins are left out, the rest of the routing still reads.
fn outside() -> Finding {
    Finding {
        file: LAYOUT_FILE.to_owned(),
        line: 1,
        col: 1,
        message: "outside the project's folders — skipped".to_owned(),
        severity: Severity::Warning,
    }
}

/// The project's pins, and the finding when the file does not read (then there are none).
pub fn read_layout(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
) -> Result<(IndexMap<String, Position>, Option<Finding>), SourceError> {
    let path = join(project, LAYOUT_FILE);
    match source.stat(&path) {
        Ok(s) if s.kind == Kind::File => {}
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => return Ok((IndexMap::new(), None)),
        Err(SourceError::Outside) => return Ok((IndexMap::new(), Some(outside()))),
        Err(e) => return Err(e),
    }
    match cache.read::<Layout>(source, &path) {
        Ok(Ok(l)) => Ok((l.positions.clone(), None)),
        Ok(Err(f)) => Ok((IndexMap::new(), Some(f.in_file(LAYOUT_FILE)))),
        Err(SourceError::Outside) => Ok((IndexMap::new(), Some(outside()))),
        Err(SourceError::NotFound) => Ok((IndexMap::new(), None)),
        Err(e) => Err(e),
    }
}
