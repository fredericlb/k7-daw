// send_refs.rs — three questions the file service asks of a project at every send: its pattern files, its plugins, its records.
// A reader would get wrong that a pattern file is a path below the project, `/`-spelled: a lane naming one outside it names none.

use crate::cache::Cache;
use crate::files::{Check, decode_with, from_map};
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::lanes::pattern_lanes;
use crate::lanes::rig::Rig;
use crate::modules::types::is_engine_ref;
use crate::modules::{LoadedModule, Plugin, builtin_scan, engine_module_of};
use crate::roots::{Roots, spell};
use crate::source::Source;
use crate::source::path::join;
use crate::views::surfaces::{SurfaceType, project_surfaces};
use serde_json::Value;
use std::collections::HashSet;

/// The formats the engine loads from its scan; `EXTERNAL_PLUGIN_FORMATS` in the engine protocol.
pub const EXTERNAL_FORMATS: [&str; 9] = ["vst3", "vst", "vst2", "au", "audiounit", "clap", "lv2", "ladspa", "aax"];

/// The folder a workspace keeps its playlists in.
const PLAYLISTS_DIR: &str = "_playlists";

/// The audio extensions, `AUDIO_EXT` in `@studio/song-schema/audioFiles`.
const AUDIO_EXT: [&str; 9] = [
    ".wav", ".flac", ".aiff", ".aif", ".mp3", ".ogg", ".opus", ".m4a", ".webm",
];

/// `recordPathOf` then `isAudio`: a line as a workspace audio file's path, or none.
fn record_of(line: &str) -> Option<String> {
    let clean = line.trim().trim_matches('/');
    if clean.is_empty() || clean.contains('\\') || clean.starts_with('@') {
        return None;
    }
    if clean.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return None;
    }
    let base = clean.rsplit('/').next().unwrap_or(clean);
    let dot = base.rfind('.').filter(|&i| i > 0)?;
    AUDIO_EXT
        .contains(&base[dot..].to_ascii_lowercase().as_str())
        .then(|| clean.to_owned())
}

fn read(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Option<Inputs> {
    load_inputs(source, cache, roots, Rig::default()).ok()
}

/// `patternFiles`: the pattern files the lanes play, once each, below the project, `/`-spelled, written yet or not.
pub fn pattern_files(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Vec<String> {
    let Some(inputs) = read(source, cache, roots) else {
        return vec![];
    };
    let lanes = inputs.lanes();
    let mut seen = HashSet::new();
    let mut out = vec![];
    for (_, strudel) in pattern_lanes(&lanes) {
        let Ok(path) = spell(&join(&inputs.project, strudel), inputs.rule) else {
            continue;
        };
        // A path outside the project's folders is no file of the project.
        let Some(rel) = path.strip_prefix(&format!("{}/", inputs.project.trim_end_matches('/'))) else {
            continue;
        };
        if seen.insert(rel.to_owned()) {
            out.push(rel.to_owned());
        }
    }
    out
}

fn is_external(m: Option<&LoadedModule>) -> bool {
    let kind = m
        .and_then(|m| m.authored.plugin.as_ref())
        .and_then(|p: &Plugin| p.plugin_type.as_deref());
    kind.is_some_and(|t| EXTERNAL_FORMATS.contains(&t.to_lowercase().as_str()))
}

/// Whether `name` reaches an external plugin by its `extends`, or stops at an engine ref nothing resolves.
fn reaches_external(name: &str, inputs: &Inputs, builtins: &HashSet<String>) -> bool {
    let unknown_engine = |r: &str| is_engine_ref(r) && !builtins.contains(r);
    let vocab = &inputs.vocabulary.modules;
    let Some(mut m) = inputs.nodes.nodes.get(name).or_else(|| vocab.get(name)) else {
        return unknown_engine(name);
    };
    let mut seen = HashSet::from([name.to_owned()]);
    loop {
        if is_external(Some(m)) {
            return true;
        }
        let Some(r) = m.authored.extends.as_deref().filter(|r| !r.is_empty()) else {
            return false;
        };
        if !seen.insert(r.to_owned()) {
            return false;
        }
        match vocab.get(r) {
            Some(parent) => m = parent,
            None => return unknown_engine(r),
        }
    }
}

/// `namesExternalPlugin`: whether the project names a plugin only the engine's scan can know; false when it does not read.
pub fn names_external_plugin(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> bool {
    let Some(inputs) = read(source, cache, roots) else {
        return false;
    };
    let resolved = inputs.local.modules.values().chain(inputs.extra.modules.values());
    if resolved.into_iter().any(|m| is_external(Some(m))) {
        return true;
    }
    let mut names: Vec<&str> = inputs.nodes.nodes.keys().map(String::as_str).collect();
    for c in inputs.chains.values() {
        names.extend(c.use_steps.iter().map(|s| s.module.as_str()));
        names.extend(c.out.iter().map(|o| o.to.as_str()));
    }
    let builtins: HashSet<String> = builtin_scan()
        .iter()
        .filter_map(|p| engine_module_of(&p.name, &p.format).map(|(r, _)| r))
        .collect();
    let mut checked = HashSet::new();
    names
        .into_iter()
        .filter(|n| !n.is_empty() && checked.insert(*n))
        .any(|n| reaches_external(n, &inputs, &builtins))
}

/// `playlistPathOf`: a playlist's name as the workspace path holding it, None for one that would leave the folder.
fn playlist_path(name: &str) -> Option<String> {
    let clean = name.trim().trim_matches('/');
    if clean.is_empty() || clean.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return None;
    }
    let ends = clean.len() >= 4
        && clean.is_char_boundary(clean.len() - 4)
        && clean[clean.len() - 4..].eq_ignore_ascii_case(".k7x");
    Some(format!("{PLAYLISTS_DIR}/{clean}{}", if ends { "" } else { ".k7x" }))
}

/// A playlist file as far as the send reads it: its `items`, whatever they hold.
#[derive(serde::Deserialize)]
struct Playlist {
    #[serde(default)]
    items: Value,
}

impl Check for Playlist {}

/// The songs of a playlist's text: a bare string, or a map's `song`; what is neither is skipped, and text that does not read has none.
fn playlist_songs(text: &str) -> Vec<String> {
    let Ok(Playlist {
        items: Value::Array(items),
    }) = decode_with("playlist", text, from_map::<Playlist>)
    else {
        return vec![];
    };
    let text_of = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    items
        .iter()
        .filter_map(|line| text_of(Some(line)).or_else(|| text_of(line.get("song"))))
        .collect()
}

/// `playlistRecords`: every workspace audio file the desk's playlist names, sorted; none when anything is unreadable.
pub fn playlist_records(source: &dyn Source, cache: &mut Cache, roots: &Roots, workspace: &str) -> Vec<String> {
    let Ok(project) = spell(&roots.project, roots.paths) else {
        return vec![];
    };
    let Ok(shown) = project_surfaces(source, cache, &project) else {
        return vec![];
    };
    let name = shown
        .surfaces
        .iter()
        .find(|s| s.surface.kind == SurfaceType::Turntables)
        .and_then(|s| s.surface.playlist.clone());
    let Some(rel) = name.as_deref().and_then(playlist_path) else {
        return vec![];
    };
    let Ok(file) = spell(&join(workspace, &rel), roots.paths) else {
        return vec![];
    };
    let Ok(text) = source.read_text(&file) else {
        return vec![];
    };
    let mut out: Vec<String> = playlist_songs(&text).iter().filter_map(|s| record_of(s)).collect();
    out.sort();
    out.dedup();
    out
}
