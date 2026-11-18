// chain_data.rs — a chain file as the editor holds it: steps of plain `Value` items, in the key order Zod gives them.
// A reader would get wrong that an item stays a map: a node's fields are a module's, so nothing here types them.

use k7x::edit::Value;
use std::collections::HashSet;

/// A step's item as written: a lane `{ id, track }`, a picture lane `{ id, video_track }` or a node.
pub type Item = Value;

fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match v.get(k) {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

fn only_keys(v: &Value, allowed: &[&str]) -> bool {
    matches!(v, Value::Map(m) if m.iter().all(|(k, _)| allowed.contains(&k.as_str())))
}

/// An item's `id`, empty when it has none.
#[must_use]
pub fn id_of(i: &Item) -> &str {
    str_of(i, "id").unwrap_or("")
}

/// A LANE item is `{ id, track, bypass? }` exactly: an input node's own `track:` field is not one.
#[must_use]
pub fn is_track_item(i: &Item) -> bool {
    str_of(i, "track").is_some() && only_keys(i, &["id", "track", "bypass"])
}

/// A PICTURE lane item, `{ id, video_track, bypass? }` exactly.
#[must_use]
pub fn is_video_item(i: &Item) -> bool {
    str_of(i, "video_track").is_some() && only_keys(i, &["id", "video_track", "bypass"])
}

#[must_use]
pub fn is_lane_item(i: &Item) -> bool {
    is_track_item(i) || is_video_item(i)
}

/// The lane an item IS, of either kind, or None for a node.
#[must_use]
pub fn lane_of(i: &Item) -> Option<&str> {
    if is_track_item(i) {
        str_of(i, "track")
    } else if is_video_item(i) {
        str_of(i, "video_track")
    } else {
        None
    }
}

/// The name a node is known by in memory, in commands and on the wire: its lane, else its id.
#[must_use]
pub fn name_of(i: &Item) -> &str {
    lane_of(i).unwrap_or_else(|| id_of(i))
}

/// Whether an item says `bypass: true`.
#[must_use]
pub fn bypassed(i: &Item) -> bool {
    matches!(i.get("bypass"), Some(Value::Bool(true)))
}

/// `item[key] = value`: the key keeps its place when the item has it, else it lands last.
pub fn set_key(item: &mut Item, key: &str, value: Value) {
    if let Value::Map(m) = item {
        match m.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => m.push((key.to_owned(), value)),
        }
    }
}

/// `delete item[key]`.
pub fn remove_key(item: &mut Item, key: &str) {
    if let Value::Map(m) = item {
        m.retain(|(k, _)| k != key);
    }
}

/// An item with its keys in the order the schema writes them out: a lane's own, then a node's four, then the module's rest.
#[must_use]
pub fn in_schema_order(item: &Item) -> Item {
    let Value::Map(m) = item else { return item.clone() };
    let first: &[&str] = if is_track_item(item) {
        &["id", "track", "bypass"]
    } else if is_video_item(item) {
        &["id", "video_track", "bypass"]
    } else {
        &["id", "bypass", "vstpreset", "mapping"]
    };
    let mut out: Vec<(String, Value)> = first
        .iter()
        .filter_map(|k| item.get(k).map(|v| ((*k).to_owned(), v.clone())))
        .collect();
    out.extend(m.iter().filter(|(k, _)| !first.contains(&k.as_str())).cloned());
    Value::Map(out)
}

/// Where the signal goes once the chain ends: another chain (a port of it) or a built-in end.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub chain: String,
    pub level: Option<f64>,
    pub port: Option<String>,
    /// Written `port` then `level`, as `from_segments` builds a link; a parsed one is `level` then `port`.
    pub port_first: bool,
}

impl Link {
    fn from_value(v: &Value) -> Link {
        Link {
            chain: str_of(v, "chain").unwrap_or("").to_owned(),
            level: match v.get("level") {
                Some(Value::Num(n)) => Some(*n),
                _ => None,
            },
            port: str_of(v, "port").map(str::to_owned),
            port_first: false,
        }
    }

    fn to_value(&self) -> Value {
        let mut out = vec![("chain".to_owned(), Value::Str(self.chain.clone()))];
        let level = self.level.map(|l| ("level".to_owned(), Value::Num(l)));
        let port = self.port.as_ref().map(|p| ("port".to_owned(), Value::Str(p.clone())));
        if self.port_first {
            out.extend(port.into_iter().chain(level));
        } else {
            out.extend(level.into_iter().chain(port));
        }
        Value::Map(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// The LAST step: links only.
    End(Vec<Link>),
    /// A subchain run in place, right there.
    Call(String),
    /// Several nodes side by side.
    Many(Vec<Item>),
    One(Item),
}

impl Step {
    /// A step told by its shape: a map with `to` is an end, with `sub` and no `id` a call, a list is parallel.
    #[must_use]
    pub fn from_value(v: &Value) -> Step {
        match v {
            Value::List(l) => Step::Many(l.iter().map(in_schema_order).collect()),
            Value::Map(_) if v.get("to").is_some() => Step::End(match v.get("to") {
                Some(Value::List(l)) => l.iter().map(Link::from_value).collect(),
                _ => vec![],
            }),
            Value::Map(_) if v.get("sub").is_some() && v.get("id").is_none() => {
                Step::Call(str_of(v, "sub").unwrap_or("").to_owned())
            }
            other => Step::One(in_schema_order(other)),
        }
    }

    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Step::End(to) => Value::Map(vec![(
                "to".to_owned(),
                Value::List(to.iter().map(Link::to_value).collect()),
            )]),
            Step::Call(sub) => Value::Map(vec![("sub".to_owned(), Value::Str(sub.clone()))]),
            Step::Many(items) => Value::List(items.clone()),
            Step::One(i) => i.clone(),
        }
    }

    /// The items of a step: none for an end or a call, several for a parallel one.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        match self {
            Step::End(_) | Step::Call(_) => &[],
            Step::Many(v) => v,
            Step::One(i) => std::slice::from_ref(i),
        }
    }

    pub fn items_mut(&mut self) -> &mut [Item] {
        match self {
            Step::End(_) | Step::Call(_) => &mut [],
            Step::Many(v) => v,
            Step::One(i) => std::slice::from_mut(i),
        }
    }

    #[must_use]
    pub fn is_end(&self) -> bool {
        matches!(self, Step::End(_))
    }

    #[must_use]
    pub fn is_call(&self) -> bool {
        matches!(self, Step::Call(_))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subchain {
    pub id: String,
    pub steps: Vec<Step>,
}

/// Where a chain starts: at its own first node (`true`), or at a sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Own,
    Mix,
    Render,
}

impl Start {
    /// The sum's name, for `MIX` and `RENDER`.
    #[must_use]
    pub fn sum(self) -> Option<&'static str> {
        match self {
            Start::Own => None,
            Start::Mix => Some("MIX"),
            Start::Render => Some("RENDER"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chain {
    pub start: Option<Start>,
    pub ports: Vec<String>,
    pub bus_only: bool,
    pub hidden: bool,
    pub input: Option<Value>,
    pub steps: Vec<Step>,
    pub subchains: Vec<Subchain>,
}

fn steps_of(v: Option<&Value>) -> Vec<Step> {
    match v {
        Some(Value::List(l)) => l.iter().map(Step::from_value).collect(),
        _ => vec![],
    }
}

impl Chain {
    /// A chain file's data, already vetted by the decoder; absent `steps` is empty.
    #[must_use]
    pub fn from_value(v: &Value) -> Chain {
        let flag = |k: &str| matches!(v.get(k), Some(Value::Bool(true)));
        Chain {
            start: match v.get("start") {
                Some(Value::Bool(true)) => Some(Start::Own),
                Some(Value::Str(s)) if s == "MIX" => Some(Start::Mix),
                Some(Value::Str(s)) if s == "RENDER" => Some(Start::Render),
                _ => None,
            },
            ports: match v.get("ports") {
                Some(Value::List(l)) => l
                    .iter()
                    .filter_map(|p| if let Value::Str(s) = p { Some(s.clone()) } else { None })
                    .collect(),
                _ => vec![],
            },
            bus_only: flag("bus_only"),
            hidden: flag("hidden"),
            input: v.get("input").cloned(),
            steps: steps_of(v.get("steps")),
            subchains: match v.get("subchains") {
                Some(Value::List(l)) => l
                    .iter()
                    .map(|sc| Subchain {
                        id: str_of(sc, "id").unwrap_or("").to_owned(),
                        steps: steps_of(sc.get("steps")),
                    })
                    .collect(),
                _ => vec![],
            },
        }
    }

    /// The chain's data in the file's key order, nothing written that says nothing.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut top: Vec<(String, Value)> = vec![];
        match self.start {
            Some(Start::Own) => top.push(("start".to_owned(), Value::Bool(true))),
            Some(s) => top.push(("start".to_owned(), Value::Str(s.sum().unwrap_or("").to_owned()))),
            None => {}
        }
        if !self.ports.is_empty() {
            let ports = self.ports.iter().map(|p| Value::Str(p.clone())).collect();
            top.push(("ports".to_owned(), Value::List(ports)));
        }
        if self.bus_only {
            top.push(("bus_only".to_owned(), Value::Bool(true)));
        }
        if self.hidden {
            top.push(("hidden".to_owned(), Value::Bool(true)));
        }
        if let Some(i) = &self.input {
            top.push(("input".to_owned(), i.clone()));
        }
        let steps = |s: &[Step]| Value::List(s.iter().map(Step::to_value).collect());
        top.push(("steps".to_owned(), steps(&self.steps)));
        if !self.subchains.is_empty() {
            let subs = self
                .subchains
                .iter()
                .map(|sc| {
                    Value::Map(vec![
                        ("id".to_owned(), Value::Str(sc.id.clone())),
                        ("steps".to_owned(), steps(&sc.steps)),
                    ])
                })
                .collect();
            top.push(("subchains".to_owned(), Value::List(subs)));
        }
        Value::Map(top)
    }

    /// Every list of steps a chain file holds: its own, then each subchain's under its id.
    pub fn step_lists(&self) -> impl Iterator<Item = (Option<&str>, &[Step])> {
        std::iter::once((None, self.steps.as_slice())).chain(
            self.subchains
                .iter()
                .map(|sc| (Some(sc.id.as_str()), sc.steps.as_slice())),
        )
    }

    /// The ids of the subchains a chain RUNS in place, by its `{ sub }` steps.
    #[must_use]
    pub fn called_subchains(&self) -> HashSet<&str> {
        self.steps
            .iter()
            .filter_map(|s| {
                if let Step::Call(id) = s {
                    Some(id.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// The chain's own steps with each `{ sub }` replaced by that subchain's steps: the run as it plays.
    #[must_use]
    pub fn expand_calls(&self) -> Vec<&Step> {
        self.steps
            .iter()
            .flat_map(|s| match s {
                Step::Call(id) => self
                    .subchains
                    .iter()
                    .find(|sc| sc.id == *id)
                    .map(|sc| sc.steps.iter().filter(|x| !x.is_end()).collect::<Vec<_>>())
                    .unwrap_or_default(),
                other => vec![other],
            })
            .collect()
    }

    /// The links the chain's last step names, none when it does not end in links.
    #[must_use]
    pub fn links(steps: &[Step]) -> &[Link] {
        match steps.last() {
            Some(Step::End(to)) => to,
            _ => &[],
        }
    }
}
