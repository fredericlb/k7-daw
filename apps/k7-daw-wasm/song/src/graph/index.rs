// graph/index.rs — the questions every pass asks of a graph, answered by indexes built once (408).
// A reader would get wrong that each was a linear `find` in a loop in the TypeScript: `chainHosting`, `isStretch`, `runOf`, `carries*`.

use super::GraphDoc;
use crate::lanes::is_builtin_output;
use crate::modules::{IoKind, ModuleKind};
use crate::routing::{Chain, split_port};
use std::collections::{HashMap, HashSet};

/// The type folder a shape lives in, and the two a picture pass may be from.
pub const SHAPE_CATEGORY: &str = "shapes";
pub const SHADER_CATEGORY: &str = "shaders";
pub const TRANSITION_CATEGORY: &str = "video_transitions";

/// What a chain carries, found by one pass over its steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Carries {
    pub camera: bool,
    pub shape: bool,
    pub midi_in: bool,
    pub input: bool,
}

/// Which materials a lane's placements hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub audio: bool,
    pub midi: bool,
}

/// A picture pass: a shader or a video effect.
fn is_picture_pass(category: Option<&str>) -> bool {
    matches!(category, Some(SHADER_CATEGORY | "video_effects"))
}

/// Indexes over one `GraphDoc`, built in a few linear passes.
pub struct Index<'g> {
    g: &'g GraphDoc<'g>,
    host: HashMap<&'g str, usize>,
    from: HashMap<&'g str, usize>,
    fed: HashSet<&'g str>,
    carries: Vec<Carries>,
    stretch: Vec<bool>,
    kinds: HashMap<&'g str, Kinds>,
}

impl<'g> Index<'g> {
    pub fn new(g: &'g GraphDoc<'_>) -> Index<'g> {
        let mut host: HashMap<&str, usize> = HashMap::new();
        let mut from: HashMap<&str, usize> = HashMap::new();
        let mut fed: HashSet<&str> = HashSet::new();
        for (i, c) in g.chains.values().enumerate() {
            for s in &c.use_steps {
                host.entry(s.module.as_str()).or_insert(i);
            }
            if let Some(f) = c.from.as_deref().filter(|f| !f.is_empty()) {
                from.entry(f).or_insert(i);
            }
            fed.extend(c.out.iter().map(|o| split_port(&o.to).0));
        }
        let carries: Vec<Carries> = g.chains.values().map(|c| carries_of(g, c)).collect();
        let stretch: Vec<bool> = g.chains.values().map(|c| stretch_of(g, c, &fed)).collect();
        let kinds = g
            .tracks
            .iter()
            .map(|(name, t)| {
                let mut k = Kinds::default();
                for p in &t.placements {
                    match p.kind {
                        super::ClipKind::Audio => k.audio = true,
                        super::ClipKind::Midi => k.midi = true,
                        _ => {}
                    }
                }
                (*name, k)
            })
            .collect();
        Index {
            g,
            host,
            from,
            fed,
            carries,
            stretch,
            kinds,
        }
    }

    pub fn graph(&self) -> &'g GraphDoc<'g> {
        self.g
    }

    /// Whether a lane was left out for being disabled (the list is sorted).
    pub fn is_disabled(&self, name: &str) -> bool {
        self.g.disabled.binary_search(&name).is_ok()
    }

    /// Whether `id` is somewhere the signal ENDS: a built-in output by name, a note, or an io that is not a source.
    pub fn is_sink(&self, id: &str) -> bool {
        if is_builtin_output(id) {
            return true;
        }
        let Some(m) = self.g.modules.get(id) else { return false };
        m.kind == ModuleKind::Doc || (m.kind == ModuleKind::Io && !m.io.is_some_and(is_source))
    }

    /// The position of the first chain whose run holds `node` as a step.
    pub fn hosting(&self, node: &str) -> Option<usize> {
        self.host.get(node).copied()
    }

    /// The position of the chain a destination continues in: itself when it names one, else the run hosting its module.
    pub fn chain_at(&self, to: &str) -> Option<usize> {
        let node = split_port(to).0;
        self.g.chains.get_index_of(node).or_else(|| self.hosting(node))
    }

    /// A destination the signal can legitimately arrive at; the port is stripped first.
    pub fn is_node(&self, id: &str) -> bool {
        let node = split_port(id).0;
        self.g.chains.contains_key(node) || self.is_sink(node) || self.hosting(node).is_some()
    }

    /// The first chain whose source is `lane`: the run that starts it.
    pub fn first_chain_from(&self, lane: &str) -> Option<&'g Chain> {
        self.from
            .get(lane)
            .and_then(|i| self.g.chains.get_index(*i))
            .map(|(_, c)| c)
    }

    /// What the chain at position `i` carries.
    pub fn carries(&self, i: usize) -> Carries {
        self.carries.get(i).copied().unwrap_or_default()
    }

    /// Whether the chain at position `i` is a stretch: picture passes nothing feeds, landing on a picture lane.
    pub fn is_stretch(&self, i: usize) -> bool {
        self.stretch.get(i).copied().unwrap_or(false)
    }

    /// Whether some chain's out lands on `node`.
    pub fn is_fed(&self, node: &str) -> bool {
        self.fed.contains(node)
    }

    /// Which materials a sound lane's placements hold.
    pub fn kinds(&self, lane: &str) -> Kinds {
        self.kinds.get(lane).copied().unwrap_or_default()
    }
}

/// The io roles the signal comes FROM.
fn is_source(io: IoKind) -> bool {
    matches!(io, IoKind::In | IoKind::File | IoKind::Camera | IoKind::Midi)
}

fn carries_of(g: &GraphDoc<'_>, c: &Chain) -> Carries {
    let mut out = Carries::default();
    for s in &c.use_steps {
        let Some(m) = g.modules.get(s.module.as_str()) else {
            continue;
        };
        let io = (m.kind == ModuleKind::Io).then_some(m.io).flatten();
        out.camera |= io == Some(IoKind::Camera);
        out.midi_in |= io == Some(IoKind::Midi);
        out.input |= io == Some(IoKind::In);
        out.shape |= m.category == Some(SHAPE_CATEGORY);
    }
    out
}

fn stretch_of(g: &GraphDoc<'_>, c: &Chain, fed: &HashSet<&str>) -> bool {
    let Some(head) = c.use_steps.first().map(|s| s.module.as_str()) else {
        return false;
    };
    if c.from.as_deref().is_some_and(|f| !f.is_empty()) {
        return false;
    }
    if !c.out.iter().any(|o| g.video_tracks.contains_key(split_port(&o.to).0)) {
        return false;
    }
    let all_passes = c
        .use_steps
        .iter()
        .all(|s| is_picture_pass(g.modules.get(s.module.as_str()).and_then(|m| m.category)));
    all_passes && !fed.contains(head)
}
