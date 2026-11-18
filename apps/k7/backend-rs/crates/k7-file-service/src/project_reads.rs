//! project_reads.rs — what a project asks of its engine and what it imported: the plugins it needs, their score against a scan, its clips (`pluginNeeds.ts`, `projectResources.ts`).
//! What a reader gets wrong: nothing here opens the project; an empty scan cannot say, which is not the same as nothing installed.

use indexmap::IndexMap;
use serde_json::{Value, json};
use song::graph::load_graph;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::modules::ModuleKind;
use song::modules::ScannedPlugin;
use song::modules::types::hosts_plugin;
use song::send_refs::EXTERNAL_FORMATS;
use song::{Cache, Roots, Source, read_family};

use crate::confine::Disk;
use crate::module_reads::Failure;
use crate::paths::resolve;
use crate::rel::to_rel;
use crate::space::{Space, is_project_dir};

/// One plugin a project asks its engine for, as authored: `format` lowercased.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    pub format: String,
    pub name: String,
}

impl Need {
    pub fn json(&self) -> Value {
        json!({ "format": self.format, "name": self.name })
    }
}

/// Every plugin the project's modules use, deduplicated, sorted; a render baked from one counts all the same.
///
/// # Errors
/// A project whose files the graph cannot load.
pub fn plugin_needs(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    rig: Rig,
) -> Result<Vec<Need>, String> {
    let inputs = load_inputs(source, cache, roots, rig).map_err(|e| e.to_string())?;
    let g = load_graph(source, cache, roots, &inputs).map_err(|e| match e {
        song::graph::GraphError::Source(e) => e.to_string(),
        song::graph::GraphError::Refused(r) => r.findings.first().map_or_else(
            || "the project does not load".to_owned(),
            |f| f.message.clone(),
        ),
    })?;
    let mut needs: IndexMap<String, Need> = IndexMap::new();
    for m in g.modules.values() {
        if m.kind != ModuleKind::Plugin || !hosts_plugin(m.category) {
            continue;
        }
        let plugin = m.m.authored.plugin.as_ref();
        let format = plugin
            .and_then(|p| p.plugin_type.clone())
            .unwrap_or_default()
            .to_lowercase();
        let name = plugin.and_then(|p| p.name.clone()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        // jsdsp, Faust and offline are built from the project's own source, on whichever engine plays it.
        if format != "internal" && !EXTERNAL_FORMATS.contains(&format.as_str()) {
            continue;
        }
        needs.insert(
            format!("{format}|{}", name.to_lowercase()),
            Need { format, name },
        );
    }
    let mut out: Vec<Need> = needs.into_values().collect();
    out.sort_by(|a, b| a.format.cmp(&b.format).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// `projectDirOf`: a project of this workspace, by the path the page names it with.
///
/// # Errors
/// A path outside the workspace, or a folder that is no project.
pub fn project_dir_of(space: &Space<'_>, project: &str) -> Result<String, Failure> {
    let dir = space.safe_path(project, &Disk)?;
    if is_project_dir(&dir) {
        Ok(dir)
    } else {
        Err(Failure::Say(format!("not a project: {project}")))
    }
}

fn key(format: &str, name: &str) -> String {
    format!("{}|{}", format.to_lowercase(), name.to_lowercase())
}

/// The needs scored against a scan; none when an EMPTY scan cannot say.
pub fn compat_of(needs: &[Need], scan: &[ScannedPlugin]) -> Option<Value> {
    if scan.is_empty() && !needs.is_empty() {
        return None;
    }
    let mut have = std::collections::HashSet::new();
    for p in scan {
        have.insert(key(&p.format, &p.name));
        if let Some(id) = p.id.as_deref().filter(|i| !i.is_empty())
            && p.format.eq_ignore_ascii_case("internal")
        {
            have.insert(key(&p.format, id));
        }
    }
    let mut wanted: IndexMap<String, &Need> = IndexMap::new();
    for n in needs {
        wanted.insert(key(&n.format, &n.name), n);
    }
    if wanted.is_empty() {
        return Some(json!({ "score": 100, "missing": [] }));
    }
    let missing: Vec<Value> = wanted
        .iter()
        .filter(|(k, _)| !have.contains(*k))
        .map(|(_, n)| n.json())
        .collect();
    let score = (wanted.len() - missing.len()) * 100 / wanted.len();
    Some(json!({ "score": score, "missing": missing }))
}

/// A take's id, `<track>_<bar.beat>_<YYMMDDTHHMMSS>`: recorded, never imported.
fn is_take(id: &str) -> bool {
    let b = id.as_bytes();
    if b.len() < 14 {
        return false;
    }
    let (head, tail) = id.split_at(b.len() - 14);
    let t = tail.as_bytes();
    let digits = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
    if !(t[0] == b'_' && digits(&t[1..7]) && t[7] == b'T' && digits(&t[8..14])) {
        return false;
    }
    let Some(at) = head.rfind('_') else {
        return false;
    };
    let number = &head[at + 1..];
    !number.is_empty()
        && number
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

fn ext_of(name: &str) -> String {
    name.rfind('.')
        .filter(|i| *i > 0)
        .map_or_else(String::new, |i| name[i..].to_lowercase())
}

fn is_video(source: &str) -> bool {
    matches!(
        ext_of(source).as_str(),
        ".mp4" | ".mov" | ".m4v" | ".webm" | ".mkv"
    )
}

fn is_audio(source: &str) -> bool {
    matches!(
        ext_of(source).as_str(),
        ".wav" | ".flac" | ".aiff" | ".aif" | ".mp3" | ".ogg" | ".opus" | ".m4a" | ".webm"
    )
}

/// The imported clips of the project at `dir`, by id; a clip file that does not read is left out.
pub fn resources(
    source: &dyn Source,
    cache: &mut Cache,
    space: &Space<'_>,
    dir: &str,
) -> Vec<Value> {
    let Ok(family) = read_family::<song::files::Clip>(source, cache, dir) else {
        return vec![];
    };
    let mut out: Vec<(String, Value)> = Vec::new();
    for (id, clip) in &family.values {
        let Ok(v) = serde_json::to_value(&**clip) else {
            continue;
        };
        let text = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let Some(src) = text("source") else {
            continue;
        };
        if is_take(id) {
            continue;
        }
        let kind = if is_video(&src) {
            "video"
        } else if is_audio(&src) {
            "audio"
        } else {
            continue;
        };
        let abs = resolve(dir, &src);
        let meta = std::fs::metadata(&abs);
        let mut row = json!({ "id": id, "kind": kind, "source": to_rel(space, &abs) });
        if let (true, Some(light)) = (kind == "video", text("proxy").or_else(|| text("optimized")))
        {
            row["preview"] = Value::String(to_rel(space, &resolve(dir, &light)));
        }
        row["bytes"] = json!(meta.as_ref().map_or(0, std::fs::Metadata::len));
        if meta.is_err() {
            row["missing"] = Value::Bool(true);
        }
        out.push((id.clone(), row));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, r)| r).collect()
}
