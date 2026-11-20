// routing/chain_file.rs — a `chains/<name>.k7x` decoded: steps, items, links, subchains, and the file's own flags.
// A reader would get wrong that a step is told by its SHAPE, by hand: a map with `to` is an end, with `sub` and no `id` a call.

use super::hand::{R, boolean, fail, finite, get, list, names, node_name, non_empty, object, require, strict};
use crate::files::{Finding, from_map, line_col, locate};
use crate::modules::{Input, Module, Monitor};
use crate::source::path::{PathRule, windows_hazard};
use k7x::{Bump, Entry, MapNode, Node, Scalar, parse_tree};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// The built-in ends a link may name beside another chain.
pub const CHAIN_SINKS: [&str; 6] = ["OUT", "MIX", "RENDER", "VIDEO_OUT", "CUE", "@out"];

/// Where a chain starts: at its own first node, or at a sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Own,
    Mix,
    Render,
}

impl Start {
    /// The sum's name for `MIX` and `RENDER`.
    pub fn sum(self) -> Option<&'static str> {
        match self {
            Start::Own => None,
            Start::Mix => Some("MIX"),
            Start::Render => Some("RENDER"),
        }
    }
}

/// A frozen chain's input: the channel and monitoring the bench's IN is set to when it opens on the chain.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainInput {
    pub channel: Input,
    #[serde(default)]
    pub monitor: Option<Monitor>,
}

/// Where the signal goes once the chain ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub chain: String,
    pub level: Option<f64>,
    pub port: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EndStep {
    pub to: Vec<Link>,
}

/// A node: a module's fields plus the four a use adds, `module` being the rest.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeItem {
    pub id: String,
    pub bypass: Option<bool>,
    pub vstpreset: Option<String>,
    pub mapping: Option<String>,
    pub module: Module,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A lane: its events live in `tracks/<track>.k7x`; `at` is where `track` is written.
    Track {
        id: String,
        track: String,
        bypass: Option<bool>,
        at: (u32, u32),
    },
    VideoTrack {
        id: String,
        video_track: String,
        bypass: Option<bool>,
    },
    Node(Box<NodeItem>),
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Track { id, .. } | Item::VideoTrack { id, .. } => id,
            Item::Node(n) => &n.id,
        }
    }

    /// The lane an item IS, of either kind.
    pub fn lane(&self) -> Option<&str> {
        match self {
            Item::Track { track, .. } => Some(track),
            Item::VideoTrack { video_track, .. } => Some(video_track),
            Item::Node(_) => None,
        }
    }

    /// The name a node is known by in memory: its lane, else its id.
    pub fn name(&self) -> &str {
        self.lane().unwrap_or_else(|| self.id())
    }

    pub fn bypass(&self) -> bool {
        match self {
            Item::Track { bypass, .. } | Item::VideoTrack { bypass, .. } => bypass.unwrap_or(false),
            Item::Node(n) => n.bypass.unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    End(EndStep),
    Call { sub: String },
    Parallel(Vec<Item>),
    One(Item),
}

impl Step {
    /// The items of a step: none for an end or a call, several for a parallel one.
    pub fn items(&self) -> &[Item] {
        match self {
            Step::End(_) | Step::Call { .. } => &[],
            Step::Parallel(v) => v,
            Step::One(i) => std::slice::from_ref(i),
        }
    }

    pub fn end(&self) -> Option<&EndStep> {
        match self {
            Step::End(e) => Some(e),
            _ => None,
        }
    }

    pub fn is_end(&self) -> bool {
        matches!(self, Step::End(_))
    }

    pub fn is_call(&self) -> bool {
        matches!(self, Step::Call { .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subchain {
    pub id: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChainFile {
    pub start: Option<Start>,
    pub ports: Option<Vec<String>>,
    pub bus_only: Option<bool>,
    pub hidden: Option<bool>,
    pub input: Option<ChainInput>,
    pub steps: Vec<Step>,
    pub subchains: Option<Vec<Subchain>>,
}

impl ChainFile {
    /// Every list of steps: the chain's own, then each subchain's under its id.
    pub fn step_lists(&self) -> impl Iterator<Item = (Option<&str>, &[Step])> {
        std::iter::once((None, self.steps.as_slice())).chain(
            self.subchains
                .iter()
                .flatten()
                .map(|s| (Some(s.id.as_str()), s.steps.as_slice())),
        )
    }

    /// The ids of the subchains this chain RUNS in place, by its `{ sub }` steps.
    pub fn called_subchains(&self) -> HashSet<&str> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                Step::Call { sub } => Some(sub.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The chain's steps with each `{ sub }` replaced by that subchain's steps, its end step left out.
    pub fn expand_calls(&self) -> Vec<&Step> {
        // Indexed once, the first of an id winning: a linear `find` per call is quadratic on 100,000 of each.
        let mut subs: HashMap<&str, &Subchain> = HashMap::new();
        for sc in self.subchains.iter().flatten() {
            subs.entry(sc.id.as_str()).or_insert(sc);
        }
        let mut out = Vec::with_capacity(self.steps.len());
        for s in &self.steps {
            match s {
                Step::Call { sub } => {
                    if let Some(sc) = subs.get(sub.as_str()) {
                        out.extend(sc.steps.iter().filter(|x| !x.is_end()));
                    }
                }
                other => out.push(other),
            }
        }
        out
    }
}

/// Whether a lane name leaves `tracks/`; on Windows a `\` or a `:` does as `/` does.
fn leaves_tracks(s: &str, windows: bool) -> bool {
    s.contains(['/', '\0']) || s == "." || s == ".." || (windows && windows_hazard(&format!("/{s}")))
}

/// A lane name: a node name that is also a file name below `tracks/`.
fn lane_name<'a>(node: &Node<'a>, rule: PathRule) -> R<&'a str> {
    let s = node_name(node)?;
    if leaves_tracks(s, rule.is_windows()) {
        return fail(
            node.start(),
            "a lane name is a file name: no `/`, `\\`, `:`, NUL, `.` or `..`",
        );
    }
    Ok(s)
}

/// A link's port: non-empty, and not ending in `#`, which would make `<chain>#<port>#` a ref with an empty half.
fn port_name(node: &Node) -> R<String> {
    let s = non_empty(node)?;
    if s.ends_with('#') {
        return fail(
            node.start(),
            "a port name does not end in `#`: neither half of a port ref may be empty",
        );
    }
    Ok(s.to_owned())
}

fn link(node: &Node) -> R<Link> {
    let m = object(node)?;
    strict(&m, &["chain", "level", "port"])?;
    let level = match get(&m, "level") {
        Some(e) => {
            let v = finite(&e.value)?;
            if v < 0.0 {
                return fail(e.value.start(), "expected a number of at least 0");
            }
            Some(v)
        }
        None => None,
    };
    Ok(Link {
        chain: non_empty(&require(&m, "chain")?.value)?.to_owned(),
        level,
        port: get(&m, "port").map(|e| port_name(&e.value)).transpose()?,
    })
}

fn item_in(text: &str, bump: &Bump, rule: PathRule, node: &Node) -> R<Item> {
    let m = object(node)?;
    let keys_within = |allowed: &[&str]| m.entries.iter().all(|e| allowed.contains(&e.key));
    let str_key = |k: &str| {
        matches!(
            get(&m, k),
            Some(Entry {
                value: Node::Scalar {
                    value: Scalar::Str(_),
                    ..
                },
                ..
            })
        )
    };
    if str_key("track") && keys_within(&["id", "track", "bypass"]) {
        strict(&m, &["id", "track", "bypass"])?;
        let track = require(&m, "track")?;
        return Ok(Item::Track {
            id: node_name(&require(&m, "id")?.value)?.to_owned(),
            track: lane_name(&track.value, rule)?.to_owned(),
            bypass: get(&m, "bypass").map(|e| boolean(&e.value)).transpose()?,
            at: line_col(text, track.key_start),
        });
    }
    if str_key("video_track") && keys_within(&["id", "video_track", "bypass"]) {
        strict(&m, &["id", "video_track", "bypass"])?;
        return Ok(Item::VideoTrack {
            id: node_name(&require(&m, "id")?.value)?.to_owned(),
            video_track: node_name(&require(&m, "video_track")?.value)?.to_owned(),
            bypass: get(&m, "bypass").map(|e| boolean(&e.value)).transpose()?,
        });
    }
    // A node: the four fields a use adds, and the rest read as a module file would be.
    let id = node_name(&require(&m, "id")?.value)?.to_owned();
    let optional = |k: &str| get(&m, k).map(|e| non_empty(&e.value).map(str::to_owned)).transpose();
    let rest: Vec<Entry> = m
        .entries
        .iter()
        .filter(|e| !["id", "bypass", "vstpreset", "mapping"].contains(&e.key))
        .copied()
        .collect();
    let bypass = get(&m, "bypass").map(|e| boolean(&e.value)).transpose()?;
    let (vstpreset, mapping) = (optional("vstpreset")?, optional("mapping")?);
    let module_map = MapNode {
        entries: bump.alloc_slice_copy(&rest),
        ..m
    };
    let mut module = from_map::<Module>(&module_map).map_err(|e| super::hand::Fail {
        at: e.at.unwrap_or(m.start),
        message: e.message,
    })?;
    // `plugin_at` is the item's own `plugin` key; the default `type: plugin` is a node FILE's and is not applied.
    module.plugin_at = get(&m, "plugin").map(|e| line_col(text, e.key_start));
    Ok(Item::Node(Box::new(NodeItem {
        id,
        bypass,
        vstpreset,
        mapping,
        module,
    })))
}

fn step_in(text: &str, bump: &Bump, rule: PathRule, node: &Node) -> R<Step> {
    match node {
        Node::List { items, .. } => {
            if items.len() < 2 {
                return fail(node.start(), "a list of items holds at least two");
            }
            let items = items
                .iter()
                .map(|n| item_in(text, bump, rule, n))
                .collect::<R<Vec<_>>>()?;
            Ok(Step::Parallel(items))
        }
        Node::Map(m) if get(m, "to").is_some() => {
            strict(m, &["to"])?;
            let to = list(&require(m, "to")?.value)?;
            if to.is_empty() {
                return fail(require(m, "to")?.value.start(), "expected at least one link");
            }
            Ok(Step::End(EndStep {
                to: to.iter().map(link).collect::<R<_>>()?,
            }))
        }
        Node::Map(m) if get(m, "sub").is_some() && get(m, "id").is_none() => {
            strict(m, &["sub"])?;
            Ok(Step::Call {
                sub: non_empty(&require(m, "sub")?.value)?.to_owned(),
            })
        }
        Node::Map(_) => item_in(text, bump, rule, node).map(Step::One),
        other => fail(other.start(), "expected a step: an object or a list of objects"),
    }
}

fn steps_in(text: &str, bump: &Bump, rule: PathRule, map: &MapNode) -> R<Vec<Step>> {
    match get(map, "steps") {
        Some(e) => list(&e.value)?.iter().map(|n| step_in(text, bump, rule, n)).collect(),
        None => Ok(Vec::new()),
    }
}

fn subchain_in(text: &str, bump: &Bump, rule: PathRule, node: &Node) -> R<Subchain> {
    let m = object(node)?;
    strict(&m, &["id", "steps"])?;
    Ok(Subchain {
        id: non_empty(&require(&m, "id")?.value)?.to_owned(),
        steps: steps_in(text, bump, rule, &m)?,
    })
}

fn read(text: &str, bump: &Bump, rule: PathRule, root: &MapNode) -> R<ChainFile> {
    strict(
        root,
        &["start", "ports", "bus_only", "hidden", "input", "steps", "subchains"],
    )?;
    let start = match get(root, "start") {
        None => None,
        Some(e) => Some(match &e.value {
            Node::Scalar {
                value: Scalar::Bool(true),
                ..
            } => Start::Own,
            Node::Scalar {
                value: Scalar::Str("MIX"),
                ..
            } => Start::Mix,
            Node::Scalar {
                value: Scalar::Str("RENDER"),
                ..
            } => Start::Render,
            other => return fail(other.start(), "expected true, \"MIX\" or \"RENDER\""),
        }),
    };
    let input = match get(root, "input") {
        Some(e) => Some(
            from_map::<ChainInput>(&object(&e.value)?).map_err(|er| super::hand::Fail {
                at: er.at.unwrap_or(e.value.start()),
                message: er.message,
            })?,
        ),
        None => None,
    };
    let subchains = match get(root, "subchains") {
        Some(e) => Some(
            list(&e.value)?
                .iter()
                .map(|n| subchain_in(text, bump, rule, n))
                .collect::<R<Vec<_>>>()?,
        ),
        None => None,
    };
    Ok(ChainFile {
        start,
        ports: get(root, "ports").map(|e| names(&e.value)).transpose()?,
        bus_only: get(root, "bus_only").map(|e| boolean(&e.value)).transpose()?,
        hidden: get(root, "hidden").map(|e| boolean(&e.value)).transpose()?,
        input,
        steps: steps_in(text, bump, rule, root)?,
        subchains,
    })
}

/// A chain file's text decoded, then its refinements checked; the first finding wins.
pub fn decode_chain(file: &str, text: &str, rule: PathRule) -> Result<ChainFile, Finding> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    let value = read(text, &bump, rule, &root).map_err(|f| f.finding(file, text))?;
    if let Some((path, message)) = super::chain_check::check(&value) {
        return Err(Finding::at(
            file,
            text,
            locate(&root, &path).unwrap_or(root.start),
            message,
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::leaves_tracks;

    #[test]
    fn a_windows_separator_leaves_tracks_on_windows_only() {
        // Decision 93: `\` and `:` are characters of a name on Unix.
        for name in ["..\\..\\x", "a\\b", "c:x", "a:b"] {
            assert!(leaves_tracks(name, true), "{name}");
            assert!(!leaves_tracks(name, false), "{name}");
        }
        assert!(leaves_tracks("a/b", false) && !leaves_tracks("gtr", true));
    }
}
