// graph_facts.rs — the little a verb asks of the project's graph: which lanes sound a picture, and which lanes are pictures.
// A reader would get wrong a graph that does not load: it answers nothing, since the verb or the compile reports it.

use crate::host::Host;
use song::Cache;
use song::graph::load_graph;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::routing::split_port;
use song_edit::loader::roots_for;
use std::collections::HashMap;

#[derive(Default)]
pub struct GraphFacts {
    /// A sound lane's `video_source:`, the picture lane whose sound it plays.
    pub video_source: HashMap<String, String>,
    pub pictures: Vec<String>,
    /// The picture lanes that are muted: bypassed in the graph or disabled in their own file.
    pub muted_pictures: Vec<String>,
}

/// The graph as `loadGraph` reads it, or None when it does not load.
pub fn graph_facts(h: &Host) -> Option<GraphFacts> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).ok()?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).ok()?;
    Some(GraphFacts {
        video_source: g
            .tracks
            .iter()
            .filter_map(|(k, t)| t.video_source.map(|v| ((*k).to_owned(), v.to_owned())))
            .collect(),
        pictures: g.video_tracks.keys().map(|k| (*k).to_owned()).collect(),
        muted_pictures: g
            .video_tracks
            .iter()
            .filter(|(_, v)| v.mute)
            .map(|(k, _)| (*k).to_owned())
            .collect(),
    })
}

/// `soundingPicture`: the picture lane `track` plays the sound of — its own `#audio` half, or a `video_source:` lane.
pub fn sounding_picture(facts: Option<&GraphFacts>, track: &str) -> Option<String> {
    let f = facts?;
    let (node, port) = split_port(track);
    if port == Some("audio") && f.pictures.iter().any(|p| p == node) {
        return Some(node.to_owned());
    }
    f.video_source.get(track).cloned()
}
