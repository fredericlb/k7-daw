// graph_read.rs — what the graph verbs read off `loadGraph`, owned: the node names, the lanes, the boxes, the pins, the cold rule.
// A reader would get wrong `module_refs`: a shared module is a node by its ref until a chain uses it, so it counts as one.

use crate::host::{Host, Thrown};
use song::Cache;
use song::graph::{GraphError, load_graph};
use song::lanes::inputs::load_inputs;
use song::lanes::is_builtin_output;
use song::lanes::rig::Rig;
use song::modules::IoKind;
use song::routing::split_port;
use song_edit::loader::roots_for;
use std::collections::HashMap;

/// The graph as the verbs ask it.
pub struct GraphView {
    pub modules: Vec<String>,
    pub module_refs: Vec<String>,
    pub tracks: Vec<String>,
    pub video_tracks: Vec<String>,
    pub assemblies: Vec<String>,
    pub positions: HashMap<String, (f64, f64)>,
    /// Each module's count of mandatory inputs, as the compile resolves it.
    pub mandatory: HashMap<String, usize>,
    cold: Vec<String>,
    file_inputs: Vec<String>,
    /// Each chain by name, the modules of its run.
    chains: Vec<(String, Vec<String>)>,
}

/// A graph that does not load, as its first finding says.
pub fn graph_error(e: GraphError) -> Thrown {
    match e {
        GraphError::Source(s) => Thrown(s.to_string()),
        GraphError::Refused(r) => Thrown(
            r.findings
                .first()
                .map_or_else(|| "the graph does not load".to_owned(), |f| f.message.clone()),
        ),
    }
}

/// `loadGraph(dir)`, or what stops it.
pub fn read_graph(h: &Host) -> Result<GraphView, Thrown> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).map_err(|e| Thrown(e.to_string()))?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).map_err(graph_error)?;
    Ok(GraphView {
        modules: g.modules.keys().map(|k| (*k).to_owned()).collect(),
        module_refs: inputs.vocabulary.modules.keys().cloned().collect(),
        tracks: g.tracks.keys().map(|k| (*k).to_owned()).collect(),
        video_tracks: g.video_tracks.keys().map(|k| (*k).to_owned()).collect(),
        assemblies: g.assemblies.keys().cloned().collect(),
        positions: g
            .positions
            .map(|p| p.iter().map(|(k, v)| (k.clone(), (v.x, v.y))).collect())
            .unwrap_or_default(),
        mandatory: g
            .modules
            .iter()
            .map(|(k, m)| ((*k).to_owned(), m.inputs.iter().filter(|i| i.mandatory).count()))
            .collect(),
        cold: g
            .modules
            .iter()
            .filter(|(_, m)| m.m.authored.cold == Some(true))
            .map(|(k, _)| (*k).to_owned())
            .collect(),
        file_inputs: g
            .modules
            .iter()
            .filter(|(_, m)| m.io == Some(IoKind::File))
            .map(|(k, _)| (*k).to_owned())
            .collect(),
        chains: g
            .chains
            .iter()
            .map(|(k, c)| (k.clone(), c.use_steps.iter().map(|s| s.module.clone()).collect()))
            .collect(),
    })
}

impl GraphView {
    fn has(list: &[String], name: &str) -> bool {
        list.iter().any(|x| x == name)
    }

    pub fn is_module(&self, name: &str) -> bool {
        Self::has(&self.modules, name)
    }

    pub fn is_track(&self, name: &str) -> bool {
        Self::has(&self.tracks, name)
    }

    pub fn is_ref(&self, name: &str) -> bool {
        Self::has(&self.module_refs, name)
    }

    pub fn is_video_track(&self, name: &str) -> bool {
        Self::has(&self.video_tracks, name)
    }

    pub fn is_assembly(&self, name: &str) -> bool {
        Self::has(&self.assemblies, name)
    }

    /// `namesANode`: a node of THIS graph, the built-in outputs and the picture lanes included.
    pub fn names_a_node(&self, name: &str) -> bool {
        is_builtin_output(name)
            || self.is_module(name)
            || self.is_ref(name)
            || self.is_track(name)
            || self.is_video_track(name)
    }

    /// `knownNodes`: the names there are, for a refusal that can be acted on.
    pub fn known_nodes(&self) -> String {
        let list = |l: &[String]| if l.is_empty() { "none".to_owned() } else { l.join(", ") };
        let mut parts = vec![format!("modules: {}", list(&self.modules))];
        if !self.video_tracks.is_empty() {
            parts.push(format!("picture lanes: {}", self.video_tracks.join(", ")));
        }
        if !self.assemblies.is_empty() {
            parts.push(format!("assemblies: {}", self.assemblies.join(", ")));
        }
        parts.join("; ")
    }

    /// `coldPlacementError`: why `name` may not run after `at`, or None.
    pub fn cold_placement_error(&self, name: &str, at: &str) -> Option<String> {
        if !Self::has(&self.cold, name) {
            return None;
        }
        let node = split_port(at).0;
        let host = self
            .chains
            .iter()
            .find(|(k, _)| k == node)
            .or_else(|| self.chains.iter().find(|(_, run)| Self::has(run, node)));
        if host.is_some_and(|(_, run)| run.iter().any(|m| Self::has(&self.file_inputs, m))) {
            return None;
        }
        let tail = if host.is_some() {
            format!(
                "The chain at '{at}' has no file input (io: file) — a live input has no \"whole\" yet. Use it in a médiathèque chain over a song, or give this chain a file source."
            )
        } else {
            format!("'{at}' is not part of a chain drawing from a file.")
        };
        Some(format!(
            "'{name}' is a COLD module: it has to know the whole of its input before it can produce any of it, so it only works on audio already on disk. {tail}"
        ))
    }
}

/// `takeSpawnAt`: a trailing `x,y` taken off the words.
pub fn take_spawn_at(parts: &mut Vec<String>) -> Option<(f64, f64)> {
    let last = parts.last()?;
    let (x, y) = last.split_once(',')?;
    let num = |s: &str| {
        let body = s.strip_prefix('-').unwrap_or(s);
        let (int, frac) = body.split_once('.').unwrap_or((body, "0"));
        let digits = |d: &str| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit());
        (digits(int) && digits(frac)).then(|| s.parse::<f64>().ok()).flatten()
    };
    let at = (num(x)?, num(y)?);
    parts.pop();
    Some(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spawn_point_is_a_trailing_pair_of_numbers() {
        let mut w: Vec<String> = ["a", "-3.5,20"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(take_spawn_at(&mut w), Some((-3.5, 20.0)));
        assert_eq!(w, ["a"]);
        let mut w: Vec<String> = ["a", "1,x"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(take_spawn_at(&mut w), None);
        assert_eq!(w.len(), 2);
    }
}
