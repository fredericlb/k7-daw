// views/hints/paths.rs — where each entity is defined, for the editor's "open the file" links: one stat per file (decision 629).
// A reader would get wrong that an entry whose name could climb out of its folder has no link, silently, and that a link out has none.

use crate::lanes::inputs::Inputs;
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use crate::views::Spend;
use indexmap::IndexMap;

/// Whether a project file is there: any kind counts, as `existsSync`; a path the Source refuses is not a link.
fn is_there(source: &dyn Source, path: &str) -> Result<bool, SourceError> {
    match source.stat(path) {
        Ok(_) => Ok(true),
        Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => Ok(false),
        Err(e) => Err(e),
    }
}

/// A name that is a path of its own: `/` anywhere, and on Windows `\` or `:`, which can climb out of a folder.
fn escapes(name: &str, rule: PathRule) -> bool {
    name.contains('/') || (rule.is_windows() && (name.contains('\\') || name.contains(':')))
}

/// The names a table is made for, borrowed from the lists the hints already hold.
pub struct Names<'a> {
    pub tracks: &'a [String],
    pub clips: &'a [String],
    pub sequences: &'a [String],
    pub chains: &'a [String],
}

/// The table of links; it holds what was paid for when the budget stops it.
pub fn paths_table(
    source: &dyn Source,
    inputs: &Inputs,
    names: &Names<'_>,
    spend: &mut Spend,
) -> Result<IndexMap<String, String>, SourceError> {
    let project = inputs.project.as_str();
    let mut paths = IndexMap::new();
    let mut add = |kind: &str, name: &str, rel: String, spend: &mut Spend| -> Result<bool, SourceError> {
        if !spend.step() {
            return Ok(false);
        }
        if escapes(name, inputs.rule) || !is_there(source, &join(project, &rel))? {
            return Ok(true);
        }
        let key = format!("{kind}:{name}");
        if !spend.bytes(key.len() + rel.len()) {
            return Ok(false);
        }
        paths.insert(key, rel);
        Ok(true)
    };
    // A module's own file: only a node has a home, and the key is the node's name.
    for (key, m) in &inputs.nodes.nodes {
        if let Some(home) = m.home.as_deref()
            && !add("module", key, format!("{home}/{key}.k7x"), spend)?
        {
            return Ok(paths);
        }
    }
    let lists = [
        ("track", "tracks", names.tracks),
        ("clip", "clips", names.clips),
        ("sequence", "sequences", names.sequences),
    ];
    for (kind, folder, list) in lists {
        for name in list {
            if !add(kind, name, format!("{folder}/{name}.k7x"), spend)? {
                return Ok(paths);
            }
        }
    }
    // Every chain is filed in `chains.k7x`: one stat answers for all of them.
    let mut chains_file = None;
    for c in names.chains {
        if !spend.step() {
            return Ok(paths);
        }
        let there = match chains_file {
            Some(t) => t,
            None => *chains_file.insert(is_there(source, &join(project, "chains.k7x"))?),
        };
        if there {
            let (key, rel) = (format!("chain:{c}"), "chains.k7x".to_owned());
            if !spend.bytes(key.len() + rel.len()) {
                return Ok(paths);
            }
            paths.insert(key, rel);
        }
    }
    Ok(paths)
}
