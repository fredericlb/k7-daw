// history_marks.rs — what a History panel row LIGHTS on the graph and the timeline: the lanes, clips, chains and nodes its files name.
// Read off the paths, project-relative, since a commit carries nothing else; a journal step's label may also name the node it moved.
// A reader would get wrong that a chain is lit whole only when the label names none of its nodes: then those nodes alone are.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// What one row touched, by the names the two surfaces draw with, each in the order it was met.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "HistoryMarkLists"))]
pub struct HistoryMark {
    pub lanes: Vec<String>,
    pub clips: Vec<String>,
    pub chains: Vec<String>,
    /// Single nodes, when the label names one of a touched chain's: then that chain is NOT lit whole.
    pub modules: Vec<String>,
}

/// The part of the page's graph the marks read: each chain's node names.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MarkGraph {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(as = "std::collections::HashMap<String, MarkChain>"))]
    pub chains: IndexMap<String, MarkChain>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MarkChain {
    #[serde(default, rename = "use")]
    pub steps: Vec<MarkStep>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct MarkStep {
    pub module: Option<String>,
}

/// A file name without its last extension.
fn stem(file: &str) -> &str {
    match file.rfind('.') {
        Some(at) if at + 1 < file.len() && !file[at + 1..].contains('/') => &file[..at],
        _ => file,
    }
}

fn add(list: &mut Vec<String>, name: &str) {
    if !list.iter().any(|n| n == name) {
        list.push(name.to_owned());
    }
}

/// The marks for one step or commit: `paths` project-relative, `label` a journal step's command line.
pub fn history_marks(paths: &[String], label: &str, graph: Option<&MarkGraph>) -> HistoryMark {
    let mut mark = HistoryMark::default();
    // A command line names its node bare or quoted: `node param "e490…" "ir" …`.
    let words: Vec<&str> = label
        .split_whitespace()
        .map(|w| {
            let w = w.strip_prefix('"').unwrap_or(w);
            w.strip_suffix('"').unwrap_or(w)
        })
        .collect();
    for p in paths {
        let (dir, file) = p.split_once('/').unwrap_or((p.as_str(), ""));
        if file.is_empty() {
            continue;
        }
        match dir {
            "tracks" => add(&mut mark.lanes, stem(file)),
            "sequences" | "clips" => add(&mut mark.clips, stem(file)),
            "chains" => {
                let chain = stem(file);
                let steps = graph
                    .and_then(|g| g.chains.get(chain))
                    .map_or(&[][..], |c| &c.steps[..]);
                let named: Vec<&str> = steps
                    .iter()
                    .filter_map(|s| s.module.as_deref())
                    .filter(|m| !m.is_empty() && words.contains(m))
                    .collect();
                if named.is_empty() {
                    add(&mut mark.chains, chain);
                } else {
                    for m in named {
                        add(&mut mark.modules, m);
                    }
                }
            }
            _ => {}
        }
    }
    mark
}

#[cfg(test)]
mod tests;
