// chain_resolve.rs — which chain a bounce runs: the project's own off the compiled doc, or a library's read as a template; after resolveChain.ts.
// A reader would get wrong a library chain's offline steps: they are cold steps for a separation, never plugins, and a chain is one or the other.

use crate::host::{Host, Thrown};
use indexmap::IndexMap;
use song::doc::{PluginDoc, ProjectDoc};
use song::modules::files::reaches_engine;
use song::modules::{LoadedModule, ModuleKind, ParamValue};
use song::resolve::MIXER_PLUGIN;
use song::source::Kind;
use song::templates::{ChainAt, Layout, Refused, StepFacts, TemplateStep, facts_of, read_template};
use song::{Resolved, Roots, Vocabulary, resolve_map};
use song_edit::js_text::js_trim;
use song_edit::loader::roots_for;
use song_edit::shared_folders::{Family, library_family_roots};
use song_edit::sugar::{is_doc_file, strip_doc_ext};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The one track a bounce document has, named so a doc read by hand says what it was for.
pub const BOUNCE_CHAIN: &str = "process";

const DEFAULT_LIBRARY: &str = "k7-core";
const GRAPH_FILE: &str = "graph.k7x";
const DSP_GRAPH_PLUGIN: &str = "DSP Graph";

/// The key a step is resolved under: no ref can spell it.
const KEY: &str = "\0step";

/// One step of a chain that writes files instead of processing a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdStep {
    pub step: String,
    pub model: String,
    pub outputs: Vec<String>,
}

/// A chain resolved far enough to render: its name for messages, the slug its renders carry, its plugins or its cold steps.
#[derive(Clone, Debug)]
pub struct ResolvedChain {
    pub ref_: String,
    pub slug: String,
    pub plugins: Vec<PluginDoc>,
    pub cold: Vec<ColdStep>,
    /// The track this chain already runs on, when it is one of the project's.
    pub own_track: Option<String>,
}

/// One chain of a library as the walk found it.
#[derive(Clone, Debug)]
struct LibraryChain {
    ref_: String,
    library: String,
    path: String,
    at: String,
    layout: Layout,
    library_root: String,
}

/// `walkChains` over one library's `chains/` folder: a file is a chain, a folder holding a graph file is one too.
fn walk(h: &Host, library: &str, root: &str, out: &mut Vec<LibraryChain>) {
    let library_root = root.strip_suffix("/chains").unwrap_or(root).to_owned();
    let mut stack: Vec<(String, String)> = vec![(root.to_owned(), String::new())];
    while let Some((abs, rel)) = stack.pop() {
        let Ok(entries) = h.fs.list(&abs) else {
            continue;
        };
        let sub = |n: &str| {
            if rel.is_empty() {
                n.to_owned()
            } else {
                format!("{rel}/{n}")
            }
        };
        if entries.iter().any(|e| e.kind == Kind::File && e.name == GRAPH_FILE) {
            // A graph file at the `chains/` root is a chain of no name: the library's walk stops.
            if !rel.is_empty() {
                out.push(LibraryChain {
                    ref_: format!("@{library}/{rel}"),
                    library: library.to_owned(),
                    path: rel.clone(),
                    at: abs.clone(),
                    layout: Layout::Folder,
                    library_root: library_root.clone(),
                });
            }
            continue;
        }
        for e in entries {
            let at = format!("{abs}/{}", e.name);
            match e.kind {
                Kind::Dir => stack.push((at, sub(&e.name))),
                Kind::File if is_doc_file(&e.name) => {
                    let path = sub(strip_doc_ext(&e.name));
                    out.push(LibraryChain {
                        ref_: format!("@{library}/{path}"),
                        library: library.to_owned(),
                        path,
                        at,
                        layout: Layout::File,
                        library_root: library_root.clone(),
                    });
                }
                Kind::File => {}
            }
        }
    }
}

/// `libraryChainDirs`: every library's chains, each ref once (a file's over a folder's), by ref in code points.
fn library_chains(h: &Host) -> Vec<LibraryChain> {
    let mut by_ref: BTreeMap<Vec<u16>, LibraryChain> = BTreeMap::new();
    let from = (!h.song_path.is_empty()).then_some(h.song_path.as_str());
    for l in library_family_roots(&*h.fs, &h.env, Family::Chains, from) {
        let mut found = Vec::new();
        walk(h, &l.name, &l.root, &mut found);
        for c in found {
            let key: Vec<u16> = c.ref_.encode_utf16().collect();
            if c.layout == Layout::File || !by_ref.contains_key(&key) {
                by_ref.insert(key, c);
            }
        }
    }
    by_ref.into_values().collect()
}

/// `chainNames`: the project's own chains, then every library's.
fn chain_names(project: &ProjectDoc, chains: &[LibraryChain]) -> Vec<String> {
    let mut own: Vec<String> = Vec::new();
    for p in project.tracks.iter().flat_map(|t| t.plugins.iter()) {
        if let Some(at) = p.id.find(':').filter(|&i| i > 0) {
            let name = p.id[..at].to_owned();
            if !own.contains(&name) {
                own.push(name);
            }
        }
    }
    own.sort();
    own.extend(chains.iter().map(|c| c.ref_.clone()));
    own
}

fn refused(r: Refused) -> String {
    match r {
        Refused::Native(m) => m,
        Refused::Limit => "the chain is too large to read here".to_owned(),
    }
}

/// `coldStepOf`: the model and the outputs of an offline step, or what to do about it.
fn cold_step_of(label: &str, f: &StepFacts) -> Result<ColdStep, String> {
    let Some(authored) = f.onnx_authored.as_deref().filter(|a| !a.is_empty()) else {
        return Err(format!(
            "step '{label}' is an offline module but names no model — it needs an `onnx:` param"
        ));
    };
    let Some(model) = f.onnx.as_deref().filter(|m| !m.is_empty()) else {
        return Err(format!(
            "step '{label}' wants the model '{authored}', which is not on this machine — put it in ~/.k7/models (or the module's own assets/)"
        ));
    };
    Ok(ColdStep {
        step: label.to_owned(),
        model: model.to_owned(),
        outputs: f.outputs.clone()?,
    })
}

/// A template step resolved against the vocabulary, as `resolveModule` hands a step over.
fn resolve_step(h: &Host, roots: &Roots, vocab: &Vocabulary, step: &TemplateStep) -> Result<Resolved, String> {
    let mut bare = LoadedModule::bare(ModuleKind::Plugin, "", &step.name);
    bare.category = None;
    bare.kind = None;
    bare.authored = step.module.clone();
    let one: IndexMap<String, LoadedModule> = std::iter::once((KEY.to_owned(), bare)).collect();
    let mut done = resolve_map(&*h.fs, roots, &one, &vocab.modules).map_err(|e| e.to_string())?;
    done.modules
        .shift_remove(KEY)
        .ok_or_else(|| format!("step '{}' does not resolve", step.label))
}

/// Only numbers reach the engine: a file-valued param travels as its own field.
fn numbers(m: &LoadedModule) -> IndexMap<String, f64> {
    let params = m.authored.params.iter().flatten();
    params
        .filter_map(|(k, v)| match v {
            ParamValue::Number(n) => Some((k.clone(), *n)),
            ParamValue::Text(_) => None,
        })
        .collect()
}

/// A live step as the plugin the bounce plays, or why this machine cannot load it.
fn step_plugin(h: &Host, roots: &Roots, vocab: &Vocabulary, step: &TemplateStep) -> Result<PluginDoc, String> {
    let m = resolve_step(h, roots, vocab, step)?;
    let plugin = m.authored.plugin.as_ref();
    let raw = plugin.and_then(|p| p.plugin_type.clone()).unwrap_or_default();
    let (kind, name) = song::console::engine_plugin(plugin, &step.label, None);
    let extends = step.module.extends.as_deref().unwrap_or("nothing");
    if kind.is_empty() || !reaches_engine(&raw) {
        return Err(format!(
            "{} (extends {extends} — not in this machine's cache)",
            step.label
        ));
    }
    let graph = m
        .jsdsp
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|src| song::jsdsp::jsdsp_graph(&*h.fs, src).ok())
        .filter(|g| {
            !matches!(
                g.state,
                song::jsdsp::GraphState::Missing | song::jsdsp::GraphState::Invalid(_)
            )
        });
    if kind == DSP_GRAPH_PLUGIN && graph.is_none() {
        return Err(format!("{} (its DSP graph has never been compiled here)", step.label));
    }
    let mut doc = PluginDoc::new(&format!("{BOUNCE_CHAIN}:{}", step.name));
    doc.plugin_type = kind;
    doc.name = name;
    doc.path = m.preset.clone().unwrap_or_default();
    doc.nam_file = m.model.clone().filter(|s| !s.is_empty());
    doc.dsp_graph = graph.map(|g| g.text.to_string());
    doc.params = numbers(&m);
    Ok(doc)
}

/// What a library chain runs: its plugins, its cold steps, and the steps this machine cannot load.
type Steps = (Vec<PluginDoc>, Vec<ColdStep>, Vec<String>);

/// `mediatekChain`: a library chain's steps that run, as plugins or cold steps, with what this machine lacks.
fn library_chain(h: &mut Host, c: &LibraryChain) -> Result<Steps, String> {
    let mut roots = roots_for(&*h.fs, &h.env, &h.song_path);
    // A cold step's model lives in the machine's models folder, which only a separation reads.
    roots.models = Some(h.env.models_root.clone());
    let at = ChainAt {
        at: &c.at,
        layout: c.layout,
        library_root: &c.library_root,
    };
    let template = read_template(&*h.fs, &at, roots.paths).map_err(refused)?;
    let vocab: Arc<Vocabulary> = h.cache.vocabulary(&*h.fs, &roots).map_err(|e| e.to_string())?;
    let (mut plugins, mut cold, mut missing) = (Vec::new(), Vec::new(), template.missing.clone());
    // A bypassed step is in the template and out of the signal, so a bounce skips it.
    for step in template.steps.iter().filter(|s| !s.bypass) {
        let facts = facts_of(&*h.fs, &roots, &vocab, step).map_err(refused)?;
        if facts.offline {
            match cold_step_of(&step.label, &facts) {
                Ok(s) => cold.push(s),
                Err(e) => missing.push(e),
            }
            continue;
        }
        match step_plugin(h, &roots, &vocab, step) {
            Ok(p) => plugins.push(p),
            Err(e) => missing.push(e),
        }
    }
    Ok((plugins, cold, missing))
}

/// `findLibraryChainBy`: by path, then by last segment, the default library first.
fn find_by<'c>(chains: &'c [LibraryChain], library: Option<&str>, want: &str) -> Option<&'c LibraryChain> {
    let mut order: Vec<&LibraryChain> = chains
        .iter()
        .filter(|c| library.is_none_or(|l| c.library == l))
        .collect();
    order.sort_by_key(|c| c.library != DEFAULT_LIBRARY);
    let by_path = order.iter().find(|c| c.path == want);
    by_path
        .or_else(|| order.iter().find(|c| c.path.rsplit('/').next() == Some(want)))
        .copied()
}

/// What a render is called after the chain: the ref's last segment, each run of other characters one `-`.
fn slug_of(want: &str) -> String {
    let last = want.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let mut out = String::new();
    let mut bad = false;
    for c in last.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            bad = false;
        } else if !bad {
            out.push('-');
            bad = true;
        }
    }
    if out.is_empty() { "chain".to_owned() } else { out }
}

/// The chain `r` names, the project's own first; the inner Err is the sentence the caller prefixes with its verb.
///
/// # Errors
/// What the vocabulary's read threw.
pub fn resolve_chain(h: &mut Host, project: &ProjectDoc, r: &str) -> Result<Result<ResolvedChain, String>, Thrown> {
    let trimmed = js_trim(r);
    if trimmed.is_empty() {
        return Ok(Err("no chain named".to_owned()));
    }
    let named = song::modules::types::library_ref_parts(trimmed);
    let want = named.map_or(trimmed, |(_, rest)| rest).trim_matches('/');
    let slug = slug_of(want);
    let prefix = format!("{trimmed}:");
    let own = project
        .tracks
        .iter()
        .find(|t| t.plugins.iter().any(|p| p.id.starts_with(&prefix)));
    if named.is_none()
        && let Some(t) = own
    {
        let plugins: Vec<PluginDoc> = t
            .plugins
            .iter()
            .filter(|p| p.id.starts_with(&prefix) && p.plugin_type != MIXER_PLUGIN)
            .map(|p| PluginDoc {
                sidechain_from: None,
                ..p.clone()
            })
            .collect();
        // A chain that is only a strip has nothing left to bake in once the mixer is dropped.
        if plugins.is_empty() {
            return Ok(Err(format!(
                "chain '{trimmed}' has no treatment to apply — only {}'s strip, which stays live",
                t.id
            )));
        }
        return Ok(Ok(ResolvedChain {
            ref_: trimmed.to_owned(),
            slug,
            plugins,
            cold: Vec::new(),
            own_track: Some(t.id.clone()),
        }));
    }
    let chains = library_chains(h);
    let Some(found) = find_by(&chains, named.map(|(l, _)| l), want).cloned() else {
        let known = chain_names(project, &chains);
        let none = if known.is_empty() { "none" } else { "" };
        let tail = if chains.is_empty() {
            " (this machine has cloned no library holding chains)"
        } else {
            ""
        };
        return Ok(Err(format!(
            "no chain '{trimmed}' — known here: {}{none}{tail}",
            known.join(", ")
        )));
    };
    let (plugins, cold, missing) = match library_chain(h, &found) {
        Ok(x) => x,
        Err(e) => return Ok(Err(e)),
    };
    let fr = &found.ref_;
    if !missing.is_empty() {
        return Ok(Err(format!(
            "chain '{fr}' has steps this machine cannot load: {}",
            missing.join(", ")
        )));
    }
    if plugins.is_empty() && cold.is_empty() {
        return Ok(Err(format!("chain '{fr}' puts the signal through no module")));
    }
    // A chain either BOUNCES or SEPARATES, and mixing the two is refused by name.
    if !cold.is_empty() && !plugins.is_empty() {
        let steps: Vec<&str> = cold.iter().map(|c| c.step.as_str()).collect();
        return Ok(Err(format!(
            "chain '{fr}' both separates ({}) and processes ({} module(s)) — a bounce cannot say which of the stems the next step treats. Put the separation in a chain of its own.",
            steps.join(", "),
            plugins.len()
        )));
    }
    Ok(Ok(ResolvedChain {
        ref_: trimmed.to_owned(),
        slug,
        plugins,
        cold,
        own_track: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::slug_of;

    #[test]
    fn a_slug_is_the_last_segment_made_safe() {
        assert_eq!(slug_of("process/normalize"), "normalize");
        assert_eq!(slug_of("amp room!!x"), "amp-room-x");
        assert_eq!(slug_of("a-!b"), "a--b");
        assert_eq!(slug_of(""), "chain");
    }
}
