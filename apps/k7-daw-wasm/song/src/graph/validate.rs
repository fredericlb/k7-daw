// graph/validate.rs — what a GraphDoc is checked for, chain by chain: sources, steps and destinations, then the passes of validate_lanes.rs.
// A reader would get wrong that nothing here stops a graph: a port mismatch THROWS before the graph exists, the rest only warns (404, 405).

use super::index::{Index, TRANSITION_CATEGORY};
use super::issues::{GraphIssue, WarningCode as C, cut, issue};
use super::{GraphDoc, MAX_ISSUES, reach, validate_lanes};
use crate::modules::{IoKind, ModuleKind, Vocabulary};
use crate::routing::{Chain, MIX_NODE, RENDER_NODE, is_parked, split_port};
use std::collections::{HashMap, HashSet};

/// The port RENDER takes the export's sound on, and the two it has.
const RENDER_AUDIO_PORT: &str = "audio";
const RENDER_PORTS: [&str; 2] = ["in", RENDER_AUDIO_PORT];
/// The separator of a node's file name in its ref, and the longest ref a hint looks up.
const NODE_SEP: char = ':';
const MAX_REF: usize = 4_096;

/// The issues a pass raises, at most `MAX_ISSUES`; past it `more` says so and nothing is built (410).
pub(super) struct Sink {
    pub issues: Vec<GraphIssue>,
    pub more: bool,
}

impl Sink {
    /// Adds the issue `f` builds, unless the cap is reached; `f` is not called then.
    pub fn add(&mut self, f: impl FnOnce() -> GraphIssue) {
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(f());
        } else {
            self.more = true;
        }
    }
}

/// Everything a pass asks: the graph, its indexes, the vocabulary, and what it has raised.
pub(super) struct Walk<'a, 'g> {
    pub g: &'g GraphDoc<'g>,
    pub idx: &'a Index<'g>,
    pub vocab: &'a Vocabulary,
    pub sink: Sink,
    /// The output names of a module, built the first time an out leaves by a port.
    outputs: HashMap<&'g str, HashSet<&'g str>>,
    /// The joined outputs of a module, cut: built once however many outs name a bad port.
    joined: HashMap<&'g str, String>,
}

/// The " — did you mean …" tail when the last `/` should have been the node separator.
fn node_hint(g: &GraphDoc<'_>, vocab: &Vocabulary, name: &str) -> String {
    let Some(at) = name.rfind('/').filter(|_| name.len() <= MAX_REF) else {
        return String::new();
    };
    let as_node = format!("{}{NODE_SEP}{}", &name[..at], &name[at + 1..]);
    if g.modules.contains_key(as_node.as_str()) || vocab.modules.contains_key(&as_node) {
        format!(
            " — did you mean '{}'? a node is named with '{NODE_SEP}', not '/'",
            cut(&as_node)
        )
    } else {
        String::new()
    }
}

fn first_input_kind(w: &Walk<'_, '_>, module: &str) -> Option<&'static str> {
    use crate::modules::PortType;
    match w.g.modules.get(module)?.inputs.first()?.kind {
        PortType::Audio => Some("audio"),
        PortType::Midi => Some("midi"),
        PortType::Video => None,
    }
}

fn is_io(w: &Walk<'_, '_>, node: &str, io: IoKind) -> bool {
    w.g.modules
        .get(node)
        .is_some_and(|m| m.kind == ModuleKind::Io && m.io == Some(io))
}

fn chain_source(w: &mut Walk<'_, '_>, name: &str, c: &Chain) {
    let g = w.g;
    let from = c.from.as_deref().filter(|f| !f.is_empty());
    let Some(from) = from else { return };
    let known = from == MIX_NODE
        || from == RENDER_NODE
        || g.tracks.contains_key(from)
        || g.video_tracks.contains_key(from)
        || (from != name && g.chains.contains_key(from));
    if !known {
        w.sink
            .add(|| issue(C::ChainFromUnknown, [("chain", cut(name)), ("from", cut(from))]));
    }
    // A DETYPED lane holding both kinds, running down one chain that expects one of them.
    let Some(head) = c
        .use_steps
        .first()
        .filter(|_| c.from_port.as_deref().is_none_or(str::is_empty))
    else {
        return;
    };
    let kinds = w.idx.kinds(from);
    if kinds.audio
        && kinds.midi
        && let Some(wants) = first_input_kind(w, &head.module)
    {
        w.sink.add(|| {
            issue(
                C::ChainMixedLane,
                [
                    ("chain", cut(name)),
                    ("track", cut(from)),
                    ("module", cut(&head.module)),
                    ("wants", wants.to_owned()),
                ],
            )
        });
    }
}

fn chain_steps(w: &mut Walk<'_, '_>, name: &str, c: &Chain) {
    let (g, vocab) = (w.g, w.vocab);
    for s in &c.use_steps {
        match w.g.modules.get(s.module.as_str()) {
            None => {
                w.sink.add(|| {
                    issue(
                        C::ChainStepUnknown,
                        [
                            ("chain", cut(name)),
                            ("module", cut(&s.module)),
                            ("hint", node_hint(g, vocab, &s.module)),
                        ],
                    )
                });
            }
            Some(m) => {
                if let Some(base) = m.extends.filter(|b| !w.vocab.modules.contains_key(*b)) {
                    w.sink.add(|| {
                        issue(
                            C::ChainExtendsUnknown,
                            [("chain", cut(name)), ("module", cut(&s.module)), ("base", cut(base))],
                        )
                    });
                } else if m.category == Some(TRANSITION_CATEGORY) {
                    w.sink.add(|| {
                        issue(
                            C::ChainTransitionWired,
                            [("chain", cut(name)), ("module", cut(&s.module))],
                        )
                    });
                }
            }
        }
    }
}

/// The names of a module's outputs, built once.
fn output_names<'a, 'g>(w: &'a mut Walk<'_, 'g>, module: &str) -> Option<&'a HashSet<&'g str>> {
    let g = w.g;
    let (key, m) = g.modules.get_key_value(module)?;
    Some(
        w.outputs
            .entry(key)
            .or_insert_with(|| m.outputs.iter().map(|o| o.name.as_str()).collect()),
    )
}

/// A module's outputs joined, cut to `NAME_CUT` characters: one join however many outs name a bad port.
fn joined_outputs(w: &mut Walk<'_, '_>, module: &str) -> String {
    let g = w.g;
    let Some((key, m)) = g.modules.get_key_value(module) else {
        return String::new();
    };
    w.joined
        .entry(key)
        .or_insert_with(|| {
            let mut s = String::new();
            for (i, o) in m.outputs.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&o.name);
                if s.len() > super::NAME_CUT * 4 {
                    break;
                }
            }
            cut(&s)
        })
        .clone()
}

fn chain_outs(w: &mut Walk<'_, '_>, name: &str, c: &Chain, ci: usize) {
    let g = w.g;
    let carries = w.idx.carries(ci);
    for o in &c.out {
        let (to, port) = split_port(&o.to);
        let out = || cut(&o.to);
        if is_io(w, to, IoKind::In) {
            w.sink
                .add(|| issue(C::OutIsInput, [("chain", cut(name)), ("out", out())]));
        } else if is_io(w, to, IoKind::Camera) {
            w.sink
                .add(|| issue(C::OutIsCamera, [("chain", cut(name)), ("out", out())]));
        } else if is_io(w, to, IoKind::Midi) {
            w.sink
                .add(|| issue(C::OutIsMidiDevice, [("chain", cut(name)), ("out", out())]));
        } else if g.video_tracks.contains_key(to) {
            if !carries.camera && !carries.shape && !w.idx.is_stretch(ci) {
                w.sink
                    .add(|| issue(C::OutIsPictureLane, [("chain", cut(name)), ("out", out())]));
            }
        } else if g.tracks.contains_key(to) || w.idx.is_disabled(to) {
            if !carries.input && !carries.midi_in {
                w.sink
                    .add(|| issue(C::OutIsTrack, [("chain", cut(name)), ("out", out())]));
            }
        } else if !w.idx.is_node(&o.to) && !carries.camera {
            w.sink
                .add(|| issue(C::OutUnknown, [("chain", cut(name)), ("out", out())]));
        }
        if let (RENDER_NODE, Some(port)) = (to, port) {
            if !RENDER_PORTS.contains(&port) {
                w.sink
                    .add(|| issue(C::OutRenderPort, [("chain", cut(name)), ("out", out())]));
            } else if port == RENDER_AUDIO_PORT && c.from.as_deref() != Some(MIX_NODE) {
                w.sink
                    .add(|| issue(C::OutRenderSoundNotMix, [("chain", cut(name)), ("out", out())]));
            }
        }
        if o.gain < 0.0 {
            w.sink
                .add(|| issue(C::OutNegativeGain, [("chain", cut(name)), ("out", out())]));
        }
        if let Some(fp) = o.from_port.as_deref().filter(|p| !p.is_empty()) {
            let src = c
                .use_steps
                .last()
                .map(|s| s.module.as_str())
                .or(c.from.as_deref().filter(|f| !f.is_empty()));
            if let Some(src) = src.filter(|s| g.modules.contains_key(*s))
                && output_names(w, src).is_some_and(|names| !names.contains(fp))
            {
                let outputs = joined_outputs(w, src);
                w.sink.add(|| {
                    issue(
                        C::OutBadPort,
                        [
                            ("chain", cut(name)),
                            ("out", out()),
                            ("source", cut(src)),
                            ("port", cut(fp)),
                            ("outputs", outputs),
                        ],
                    )
                });
            }
        }
    }
    if c.out.is_empty() && !is_parked(c) {
        w.sink.add(|| issue(C::ChainDeadEnd, [("chain", cut(name))]));
    }
}

/// `validate`'s issues and whether more were left out past `MAX_ISSUES` (410), in the TypeScript's order.
pub fn validate_inner(g: &GraphDoc<'_>, vocab: &Vocabulary) -> (Vec<GraphIssue>, bool) {
    let idx = Index::new(g);
    // The index borrows the graph for as long as the walk does.
    let g = idx.graph();
    let mut w = Walk {
        g,
        idx: &idx,
        vocab,
        sink: Sink {
            issues: Vec::new(),
            more: false,
        },
        outputs: HashMap::new(),
        joined: HashMap::new(),
    };
    for (ci, (name, c)) in g.chains.iter().enumerate() {
        chain_source(&mut w, name, c);
        chain_steps(&mut w, name, c);
        chain_outs(&mut w, name, c, ci);
    }
    validate_lanes::lanes(&mut w);
    // Every source chain (has a track input) should reach a sink.
    let reaches = reach::reaching_sink(&idx);
    for (ci, (name, c)) in g.chains.iter().enumerate() {
        if c.from.as_deref().is_some_and(|f| !f.is_empty()) && !reaches[ci] {
            w.sink.add(|| issue(C::ChainNoOut, [("chain", cut(name))]));
        }
    }
    if let Some((name, _)) = reach::first_cycle(&idx).and_then(|i| g.chains.get_index(i)) {
        w.sink.add(|| issue(C::ChainCycle, [("chain", cut(name))]));
    }
    (w.sink.issues, w.sink.more)
}

/// `validate`'s issues alone (410).
pub fn validate_issues(g: &GraphDoc<'_>, vocabulary: &Vocabulary) -> Vec<GraphIssue> {
    validate_inner(g, vocabulary).0
}
