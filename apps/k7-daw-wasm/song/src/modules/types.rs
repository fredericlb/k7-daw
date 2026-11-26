// modules/types.rs — the module vocabulary: type folders and their default ports, how a module is keyed, named and slugged.
// A reader would get wrong that the type folder is kept in an io module's key (`@ns/io/name`), where the bare ref drops it.

use super::{ModuleKind, Port, PortType};
use crate::files::scalar::js_space;

pub const NODE_SEP: char = ':';
pub const META_NS: &str = "@meta";
pub const ENGINE_NS: &str = "@engine";
pub const ENGINES_DIR: &str = "engines";
pub const MODULE_FILE: &str = "module";
pub const COLD_TYPE: &str = "cold";
pub const CONSOLE_TYPE: &str = "console";
pub const TRACK_CATEGORY: &str = "tracks";
pub const DOC_CATEGORY: &str = "docs";
pub const NOT_SUPPORTED_CATEGORY: &str = "misc";
pub const DOC_REF: &str = "@meta/doc";
pub const NOT_SUPPORTED_REF: &str = "@meta/not-supported";
pub const NOT_SUPPORTED_TYPE: &str = "not_supported";
pub const TRACK_REFS: [&str; 4] = [
    "@meta/track",
    "@meta/strudel-track",
    "@meta/group-track",
    "@meta/picture-track",
];
pub const SHARED_MODULES_DIR: &str = "_modules";
pub const JSDSP_TYPE: &str = "jsdsp";
pub const JSDSP_FILE: &str = "jsdsp.ts";
pub const DSP_TYPE: &str = "dsp";
pub const DSP_FILE: &str = "dsp.k7x";

/// The module folder's own basenames: anything else beside `module.k7x` is a node.
pub const MODULE_OWN_BASES: [&str; 5] = ["module", "ui", "presets", "controller", "dsp"];

/// The plugin formats the engine loads, lowercase.
pub const KNOWN_PLUGIN_TYPES: [&str; 10] = [
    "internal",
    "vst3",
    "vst",
    "vst2",
    "au",
    "audiounit",
    "clap",
    "lv2",
    "ladspa",
    "aax",
];

/// What a module of one type folder has when it says nothing itself.
pub struct TypeMeta {
    pub name: &'static str,
    pub kind: ModuleKind,
    pub inputs: &'static [(&'static str, PortType, Option<bool>)],
    pub outputs: &'static [(&'static str, PortType, Option<bool>)],
}

use PortType::{Audio, Midi, Video};

pub use super::type_table::{IO_MODULES, IO_SOURCES, MODULE_TYPES};

pub fn type_meta(name: &str) -> Option<&'static TypeMeta> {
    MODULE_TYPES.iter().find(|t| t.name == name)
}

/// The ports a type folder gives, as owned values.
pub fn ports(list: &[(&str, PortType, Option<bool>)]) -> Vec<Port> {
    list.iter()
        .map(|(name, kind, mandatory)| Port {
            name: (*name).to_owned(),
            kind: *kind,
            mandatory: *mandatory,
        })
        .collect()
}

pub fn meta_ref(slug: &str) -> String {
    format!("{META_NS}/{slug}")
}

pub fn engine_ref(slug: &str) -> String {
    format!("{ENGINE_NS}/{slug}")
}

pub fn engine_ref_of(module_ref: &str, engine: &str) -> String {
    format!("{module_ref}/{ENGINES_DIR}/{engine}")
}

/// `module:node`, or the bare ref.
pub fn module_key(module: &str, node: Option<&str>) -> String {
    match node {
        Some(n) if !n.is_empty() => format!("{module}{NODE_SEP}{n}"),
        _ => module.to_owned(),
    }
}

/// Splits on the FIRST separator, so it stays total on any key.
pub fn split_module_key(key: &str) -> (&str, Option<&str>) {
    match key.split_once(NODE_SEP) {
        Some((m, n)) => (m, Some(n)),
        None => (key, None),
    }
}

/// Whether a ref is the engine's: the namespace itself or anything under it.
pub fn is_engine_ref(r: &str) -> bool {
    r == ENGINE_NS || r.strip_prefix(ENGINE_NS).is_some_and(|rest| rest.starts_with('/'))
}

/// A plugin's name as a ref-safe slug: lowercased, runs of anything else one `-`, none at either end.
pub fn slugify(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut dash = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

/// The type folder a scanned plugin belongs in; it decides the ports.
pub fn category_for_role(role: Option<&str>, instrument: bool) -> &'static str {
    match role {
        Some("midiEffect") => "transformers",
        Some("instrument") => "instruments",
        _ if instrument => "instruments",
        _ => "audio",
    }
}

pub const MODULE_STATUSES: [&str; 3] = ["listed", "favorite", "hidden"];

/// A module's status for the completions: `listed` for anything that does not say otherwise.
pub fn module_status_of(status: Option<&str>) -> &'static str {
    match status {
        Some("hidden") => "hidden",
        Some("favorite") => "favorite",
        _ => "listed",
    }
}

/// A list with the hidden dropped and the favourites floated up, stable within each group.
pub fn completion_order<T>(items: Vec<T>, status_of: impl Fn(&T) -> &'static str) -> Vec<T> {
    let (mut favorite, mut listed) = (Vec::new(), Vec::new());
    for it in items {
        match status_of(&it) {
            "hidden" => {}
            "favorite" => favorite.push(it),
            _ => listed.push(it),
        }
    }
    favorite.append(&mut listed);
    favorite
}

/// What a library may be called: `[a-z0-9][a-z0-9-]{0,39}`, and not a name a ref already means something by.
pub fn is_library_name(name: &str) -> bool {
    let b = name.as_bytes();
    let shape = !b.is_empty()
        && b.len() <= 40
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-');
    shape && !["engine", "workspace", "account", "meta", "studio", "mediatek"].contains(&name)
}

/// `@<library>/<rest>` as its library and the rest, or None for a ref no library answers to.
pub fn library_ref_parts(r: &str) -> Option<(&str, &str)> {
    let rest = r.strip_prefix('@')?;
    let (lib, tail) = rest.split_once('/')?;
    // The first segment is `[^/]+` and the tail `.+`: the greedy first slash is the first one.
    (!lib.is_empty() && !tail.is_empty() && (is_library_name(lib) || lib == "account")).then_some((lib, tail))
}

/// `[A-Za-z0-9_-]+`, the shape of an engine, a shader, a tap and a pad name.
pub fn is_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// JS's `\S+` to the end of `s`.
pub fn all_non_space(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(js_space)
}

/// The io source roles: the signal comes FROM them.
pub fn is_io_source(io: &str) -> bool {
    IO_SOURCES.contains(&io)
}

/// The signal a module of a type folder emits, from its io role or its ports.
pub fn module_emits(category: Option<&str>, io: Option<&str>, outputs: &[Port]) -> Option<PortType> {
    match category {
        Some("io") => match io {
            Some("camera") => Some(Video),
            Some("midi") => Some(Midi),
            Some(r) if is_io_source(r) => Some(Audio),
            _ => None,
        },
        Some(TRACK_CATEGORY) => {
            let first = outputs.first()?.kind;
            outputs.iter().all(|o| o.kind == first).then_some(first)
        }
        Some("instruments" | "audio" | "console" | COLD_TYPE) => Some(Audio),
        Some("transformers" | "midi") => Some(Midi),
        Some("video_effects" | "video_transitions" | "shaders" | "shapes") => Some(Video),
        _ => None,
    }
}

/// The type folders whose modules are their behaviour, so an empty `plugin:` is no hole.
pub fn hosts_plugin(category: Option<&str>) -> bool {
    !matches!(category, Some("shaders" | "shapes" | "video_transitions"))
}

/// The type folders whose modules are a PASS over a picture: what a lane run, the frame and a fade stretch are made of.
pub fn is_picture_pass(category: Option<&str>) -> bool {
    matches!(category, Some("shaders" | "video_effects"))
}
