// glsl.rs — the GLSL a shader or a transition module carries, read off its folder or an ancestor's, nearest first.
// A reader would get wrong where the stages sit: a shader's under `shaders/<name>/`, a transition's loose in its folder.

use super::doc::{Glsl, Transition};
use indexmap::IndexMap;
use song::modules::LoadedModule;
use song::{Kind, Source, Span};

/// How many `extends` a walk climbs: a cycle is refused upstream, this is the backstop.
const MAX_ANCESTRY: usize = 16;
const VERTEX: &str = "vertex.glsl";
const FRAGMENT: &str = "fragment.glsl";
pub const SHADER_PARAMS: [&str; 4] = ["P1", "P2", "P3", "P4"];

fn join(a: &str, b: &str) -> String {
    format!("{}/{}", a.trim_end_matches('/'), b)
}

/// Letters, digits, `_` and `-`: refused before any path is built, since `../` climbs out of a module.
fn is_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The module folders to look in, nearest first, off the vocabulary.
pub fn owner_folders(dir: &str, m: &LoadedModule, refs: &IndexMap<String, LoadedModule>) -> Vec<String> {
    let mut out = Vec::new();
    match (m.folder_abs.as_deref().filter(|f| !f.is_empty()), m.category.as_deref()) {
        (Some(f), _) => out.push(f.to_owned()),
        (None, Some(c)) if !m.file.is_empty() => out.push(join(&join(&join(dir, "modules"), c), &m.file)),
        _ => {}
    }
    let mut next = m.authored.extends.clone();
    for _ in 0..MAX_ANCESTRY {
        let Some(r) = next.filter(|r| !r.is_empty()) else { break };
        let Some(parent) = refs.get(&r) else { break };
        if let Some(f) = parent.folder_abs.as_deref().filter(|f| !f.is_empty()) {
            out.push(f.to_owned());
        }
        next = parent.authored.extends.clone();
    }
    out
}

fn is_file(fs: &dyn Source, path: &str) -> bool {
    fs.stat(path).is_ok_and(|s| s.kind == Kind::File)
}

fn text(fs: &dyn Source, path: &str) -> Option<String> {
    fs.read(path, Span::All).ok().and_then(|b| String::from_utf8(b).ok())
}

/// Both stages of the folder, or none: half a program would fail in GL rather than in a file.
fn stages(fs: &dyn Source, folder: &str) -> Option<Option<(String, String)>> {
    let (v, f) = (join(folder, VERTEX), join(folder, FRAGMENT));
    if !is_file(fs, &v) || !is_file(fs, &f) {
        return None;
    }
    Some(text(fs, &v).zip(text(fs, &f)))
}

/// One shader's stages, or none when the name carries nothing.
pub fn read_shader(
    fs: &dyn Source,
    dir: &str,
    m: &LoadedModule,
    refs: &IndexMap<String, LoadedModule>,
    name: &str,
) -> Option<Glsl> {
    if !is_name(name) {
        return None;
    }
    for folder in owner_folders(dir, m, refs) {
        if let Some(found) = stages(fs, &join(&join(&folder, "shaders"), name)) {
            return found.map(|(vertex, fragment)| Glsl {
                name: name.to_owned(),
                vertex,
                fragment,
            });
        }
    }
    None
}

/// The four knobs, each clamped to 0..1 since it leaves for GLSL.
pub fn knobs(params: Option<&IndexMap<String, song::modules::ParamValue>>) -> IndexMap<String, f64> {
    SHADER_PARAMS
        .iter()
        .map(|p| {
            let n = match params.and_then(|m| m.get(*p)) {
                Some(song::modules::ParamValue::Number(n)) if n.is_finite() => *n,
                _ => 0.0,
            };
            ((*p).to_owned(), n.clamp(0.0, 1.0))
        })
        .collect()
}

/// A transition a cut names: the nearest module of `video_transitions/` by that name, its stages loose in its folder.
pub fn read_transition(
    fs: &dyn Source,
    dir: &str,
    name: &str,
    refs: &IndexMap<String, LoadedModule>,
) -> Option<Transition> {
    if !is_name(name) {
        return None;
    }
    let tail = format!("video_transitions/{name}");
    let m = refs.get(&tail).or_else(|| {
        refs.iter()
            .find(|(k, _)| k.strip_suffix(&tail).is_some_and(|h| h.ends_with('/')))
            .map(|(_, m)| m)
    })?;
    for folder in owner_folders(dir, m, refs) {
        if let Some(found) = stages(fs, &folder) {
            return found.map(|(vertex, fragment)| Transition {
                name: name.to_owned(),
                vertex,
                fragment,
                params: knobs(m.authored.params.as_ref()),
            });
        }
    }
    None
}
