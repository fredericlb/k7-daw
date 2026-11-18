// views/fields.rs — the editor's field vocabulary: which kind of file a path is, and the keys and values it offers.
// A reader would get wrong where the keys come from: each derived decoder names its own, so the two cannot drift.

use crate::files::{AudioClip, FadeMap, Note, Placement, ProjectFile, ProjectSettings, Sequence, VideoClip, Warp};
use crate::modules::{Alternative, Module, RackSound, Tab, Ui};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use std::cell::Cell;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum FileKind {
    Module,
    Chains,
    Chain,
    Clip,
    Sequence,
    Curve,
    Project,
}

const LINE_ENDS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

/// Whether `p` holds `dir/` at its start or after a `/`.
fn has_dir(p: &str, dir: &str) -> bool {
    let tail = format!("{dir}/");
    p.starts_with(&tail) || p.contains(&format!("/{tail}"))
}

/// The offsets just past each `chains/` that starts `p` or follows a `/`.
fn chains_ends(p: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = p[from..].find("chains/") {
        let at = from + i;
        if at == 0 || p.as_bytes()[at - 1] == b'/' {
            out.push(at + "chains/".len());
        }
        from = at + 1;
    }
    out
}

/// Which kind a project-relative path is, by FOLDER and not by extension.
pub fn file_kind_for(path: &str) -> Option<FileKind> {
    let p = path.replace('\\', "/");
    if p.is_empty() {
        return None;
    }
    if has_dir(&p, "modules") || has_dir(&p, "nodes") || has_dir(&p, "tracks") {
        return Some(FileKind::Module);
    }
    // The chain-file rules ignore case whole, so they read a lowered copy (ASCII only, as a JS flag).
    let lower = p.to_ascii_lowercase();
    let last_end = lower.rfind(LINE_ENDS).map_or(0, |i| i + 1);
    let ends = chains_ends(&lower);
    // The rest after `chains/` may hold no line end, which `.` does not match.
    let usable = |e: &usize| *e >= last_end;
    if ends.iter().filter(|e| usable(e)).any(|e| {
        let rest = &lower[*e..];
        rest == "graph.k7x" || (rest.ends_with("/graph.k7x") && rest.len() > "/graph.k7x".len())
    }) {
        return Some(FileKind::Chains);
    }
    if ends.iter().filter(|e| usable(e)).any(|e| {
        let rest = &lower[*e..];
        rest.ends_with(".k7x") && rest.len() > ".k7x".len()
    }) {
        return Some(FileKind::Chain);
    }
    for (dir, kind) in [
        ("clips", FileKind::Clip),
        ("sequences", FileKind::Sequence),
        ("curves", FileKind::Curve),
    ] {
        if has_dir(&p, dir) {
            return Some(kind);
        }
    }
    let named = |name: &str| lower == name || lower.ends_with(&format!("/{name}"));
    if named("chains.k7x") {
        return Some(FileKind::Chains);
    }
    if named("project.k7x") {
        return Some(FileKind::Project);
    }
    None
}

type Names = &'static [&'static str];

struct Captured(());

impl fmt::Debug for Captured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("captured")
    }
}

impl fmt::Display for Captured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("captured")
    }
}

impl std::error::Error for Captured {}

impl de::Error for Captured {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Captured(())
    }
}

/// A deserializer that answers a struct request with the names the derive passed, and nothing else.
struct Probe<'a>(&'a Cell<Option<Names>>);

impl<'de> Deserializer<'de> for Probe<'_> {
    type Error = Captured;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Captured> {
        Err(Captured(()))
    }

    fn deserialize_struct<V: Visitor<'de>>(self, _: &'static str, fields: Names, _: V) -> Result<V::Value, Captured> {
        self.0.set(Some(fields));
        Err(Captured(()))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option
        unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier ignored_any
    }
}

/// The wire names a derived struct reads.
fn names_of<T: for<'de> Deserialize<'de>>() -> Names {
    let seen = Cell::new(None);
    let _ = T::deserialize(Probe(&seen));
    seen.get().unwrap_or(&[])
}

fn union(lists: &[Names]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for n in lists.iter().flat_map(|l| l.iter()) {
        if !out.contains(n) {
            out.push(n);
        }
    }
    out
}

// Hand-dispatched decoders name their keys here; the lattice holds each against the TypeScript.
const WIDGET_FIELDS: Names = &[
    "type", "col", "row", "colSpan", "rowSpan", "label", "hidden", "text", "bind", "min", "max", "step", "curve",
    "source", "options", "library",
];
const RACK_FIELDS: Names = &["rack_type", "source", "symbols"];
const CHAIN_FILE_FIELDS: Names = &["start", "ports", "bus_only", "hidden", "input", "steps", "subchains"];
const SUBCHAIN_FIELDS: Names = &["id", "steps"];
const LINK_FIELDS: Names = &["chain", "level", "port"];
const NODE_ITEM_FIELDS: Names = &["id", "bypass", "vstpreset", "mapping"];
const GRAPH_FIELDS: Names = &["modules", "bypassed", "bus_only", "tap_feeds", "positions"];
// The curve kinds' own derives are private to curves/, so their keys are named once here.
const CURVE_FIELDS: Names = &[
    "type",
    "polarity",
    "amplitude",
    "cut",
    "conditions",
    "shape",
    "frequency",
    "phase",
    "duration",
    "points",
    "attack",
    "decay",
    "sustain",
    "release",
    "level",
];

const MODULE_TYPES: Names = &["plugin", "rack", "video_effect", "video_transition"];
const IO_KINDS: Names = &["out", "in", "video_out", "file", "camera", "midi"];
const SHAPE_KINDS: Names = &["square", "circle", "text"];
const WIDGET_TYPES: Names = &[
    "label",
    "knob",
    "slider",
    "fader",
    "number",
    "toggle",
    "button",
    "meter",
    "waveform",
    "dropdown",
    "filepicker",
    "lcdscreen",
    "tap",
    "group",
];

fn module_fields(path: &[&str]) -> Vec<&'static str> {
    let Some((head, rest)) = path.split_first() else {
        return names_of::<Module>().to_vec();
    };
    match *head {
        "events" => match rest.first() {
            Some(&("fadeIn" | "fadeOut")) => names_of::<FadeMap>().to_vec(),
            _ => names_of::<Placement>().to_vec(),
        },
        "alternatives" if rest.is_empty() => Vec::new(),
        "alternatives" => names_of::<Alternative>().to_vec(),
        "ui" => match rest {
            [] => names_of::<Ui>().to_vec(),
            ["widgets"] => Vec::new(),
            ["widgets", ..] => WIDGET_FIELDS.to_vec(),
            ["tabs", _] => names_of::<Tab>().to_vec(),
            ["tabs", _, "widgets"] => Vec::new(),
            ["tabs", _, "widgets", ..] => WIDGET_FIELDS.to_vec(),
            _ => Vec::new(),
        },
        "rack" => match rest {
            [] => RACK_FIELDS.to_vec(),
            ["symbols"] => Vec::new(),
            ["symbols", ..] => names_of::<RackSound>().to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn step_fields(rest: &[&str], lanes: bool) -> Vec<&'static str> {
    match rest {
        [] => {
            let mut own: Vec<&'static str> = vec!["to"];
            if lanes {
                own.push("track");
            }
            own.extend(NODE_ITEM_FIELDS);
            own.extend(names_of::<Module>());
            let mut seen: Vec<&'static str> = Vec::new();
            for n in own {
                if !seen.contains(&n) {
                    seen.push(n);
                }
            }
            seen
        }
        ["to"] => LINK_FIELDS.to_vec(),
        ["to", ..] => Vec::new(),
        _ => module_fields(rest),
    }
}

fn chain_file_fields(path: &[&str]) -> Vec<&'static str> {
    match path {
        [] => CHAIN_FILE_FIELDS.to_vec(),
        ["steps", rest @ ..] => {
            let mut own = step_fields(rest, true);
            if rest.is_empty() {
                own.push("sub");
            }
            own
        }
        ["subchains"] => SUBCHAIN_FIELDS.to_vec(),
        ["subchains", "steps", rest @ ..] => step_fields(rest, false),
        _ => Vec::new(),
    }
}

/// The fields allowed in the mapping at `path`, the editor's key path (`[]` at the top level).
pub fn fields_at(kind: FileKind, path: &[&str]) -> Vec<&'static str> {
    match kind {
        FileKind::Module => module_fields(path),
        FileKind::Chains => {
            if path.is_empty() {
                GRAPH_FIELDS.to_vec()
            } else {
                Vec::new()
            }
        }
        FileKind::Chain => chain_file_fields(path),
        FileKind::Project => match path {
            [] => names_of::<ProjectFile>().to_vec(),
            ["project"] => names_of::<ProjectSettings>().to_vec(),
            _ => Vec::new(),
        },
        FileKind::Clip => match path {
            [] => union(&[names_of::<AudioClip>(), names_of::<VideoClip>()]),
            ["warp", ..] => names_of::<Warp>().to_vec(),
            _ => Vec::new(),
        },
        FileKind::Sequence => match path {
            [] => names_of::<Sequence>().to_vec(),
            ["notes", ..] => names_of::<Note>().to_vec(),
            _ => Vec::new(),
        },
        FileKind::Curve => match path {
            [] => CURVE_FIELDS.to_vec(),
            ["points", ..] => vec!["time", "value"],
            _ => Vec::new(),
        },
    }
}

/// The values allowed for a field whose schema is a closed set; empty for free text.
pub fn values_at(kind: FileKind, path: &[&str], field: &str) -> Vec<&'static str> {
    let root = path.is_empty();
    match kind {
        FileKind::Module => {
            let head = path.first().copied();
            match (root, head, field) {
                (true, _, "type") => MODULE_TYPES.to_vec(),
                (true, _, "io") => IO_KINDS.to_vec(),
                (true, _, "shape") => SHAPE_KINDS.to_vec(),
                (true, _, "monitor") => vec!["off", "auto", "on"],
                (_, Some("ui"), "type") => WIDGET_TYPES.to_vec(),
                (_, Some("ui"), "curve") => vec!["lin", "log"],
                (_, Some("ui"), "source") => vec!["input", "output"],
                (_, Some("rack"), "rack_type") => vec!["samples", "slice"],
                _ => Vec::new(),
            }
        }
        FileKind::Curve => match (root, field) {
            (true, "type") => vec!["lfo", "custom", "adsr", "onoff"],
            (true, "polarity") => vec!["unipolar", "bipolar"],
            (true, "shape") => vec!["sine", "triangle", "saw", "square"],
            _ => Vec::new(),
        },
        FileKind::Chain => match path {
            [] if field == "start" => vec!["true", "MIX", "RENDER"],
            ["steps", rest @ ..] => values_at(FileKind::Module, rest, field),
            ["subchains", "steps", rest @ ..] => values_at(FileKind::Module, rest, field),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}
