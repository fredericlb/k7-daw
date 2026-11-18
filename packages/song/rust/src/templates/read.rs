// templates/read.rs — `read_template`: a file chain by `decode_chain`, a folder chain by its graph and its node files.
// A reader would get wrong what a missing node is: absent or unparsable is "missing"; a read that failed is a refusal.

use super::{ChainAt, GRAPH_FILE, Layout, MAX_CHAIN_BYTES, MAX_STEPS, NODES_DIR, Refused, Template, TemplateStep};
use crate::files::{Finding, decode};
use crate::modules::{Module, VSTPRESETS_DIR};
use crate::roots::spell;
use crate::routing::{CHAIN_IN, CHAIN_OUT, Item, Step, decode_chain, decode_graph, derive_chains};
use crate::source::path::{PathRule, join};
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexSet;
use k7x::{Bump, parse_tree};

fn sentence(f: &Finding) -> String {
    format!("{}:{} {}", f.line, f.col, f.message)
}

/// Whether a path exists, a folder included; only an absent path is false, so a failed stat refuses.
fn exists(source: &dyn Source, path: &str) -> Result<bool, Refused> {
    match source.stat(path) {
        Ok(_) => Ok(true),
        Err(SourceError::NotFound) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// A template's steps in order; refused when its file does not read or is not a template's shape.
pub fn read_template(source: &dyn Source, c: &ChainAt<'_>, rule: PathRule) -> Result<Template, Refused> {
    match c.layout {
        Layout::File => read_file(source, c, rule),
        Layout::Folder => read_folder(source, c, rule),
    }
}

fn read_file(source: &dyn Source, c: &ChainAt<'_>, rule: PathRule) -> Result<Template, Refused> {
    if source.stat(c.at)?.len > MAX_CHAIN_BYTES {
        return Err(Refused::Limit);
    }
    let text = source.read_text(c.at)?;
    let value = decode_chain(c.at, &text, rule).map_err(|f| Refused::Native(sentence(&f)))?;
    if value.start.is_some() || value.ports.as_ref().is_some_and(|p| !p.is_empty()) {
        return Err(Refused::Native(
            "a template has no `start` and no `ports`: its one input is what it is put after".to_owned(),
        ));
    }
    if value.subchains.as_ref().is_some_and(|s| !s.is_empty()) {
        return Err(Refused::Native(
            "a template is one run: it holds no `subchains`".to_owned(),
        ));
    }
    if value.steps.len() > MAX_STEPS {
        return Err(Refused::Limit);
    }
    let mut steps = Vec::new();
    for (i, s) in value.steps.iter().enumerate() {
        match s {
            Step::End(end) => {
                let strays: Vec<String> = end
                    .to
                    .iter()
                    .filter(|l| l.chain != CHAIN_OUT)
                    .map(|l| format!("\"{}\"", l.chain))
                    .collect();
                if !strays.is_empty() {
                    return Err(Refused::Native(format!(
                        "a template links only to \"{CHAIN_OUT}\", not {}",
                        strays.join(", ")
                    )));
                }
            }
            Step::Parallel(_) => {
                return Err(Refused::Native(format!(
                    "step {} is side by side — a template is one run, step after step",
                    i + 1
                )));
            }
            Step::Call { .. } | Step::One(Item::Track { .. } | Item::VideoTrack { .. }) => {
                return Err(Refused::Native(format!("step {} is not a node", i + 1)));
            }
            Step::One(Item::Node(n)) => {
                let name = n.vstpreset.clone().unwrap_or_else(|| format!("{}.vstpreset", n.id));
                let at = spell(&format!("{}/{VSTPRESETS_DIR}/{name}", c.library_root), rule)?;
                steps.push(TemplateStep {
                    name: n.id.clone(),
                    label: n.module.extends.clone().unwrap_or_else(|| n.id.clone()),
                    module: n.module.clone(),
                    bypass: n.bypass.unwrap_or(false),
                    preset: exists(source, &at)?.then_some(at),
                });
            }
        }
    }
    Ok(Template {
        steps,
        missing: Vec::new(),
        input: value.input,
    })
}

/// The module a node file holds: None when it is absent or does not parse, a refusal when it parses and is not a module.
fn node_module(source: &dyn Source, file: &str) -> Result<Option<Module>, Refused> {
    match source.stat(file) {
        Ok(st) if st.kind == Kind::Dir => return Ok(None),
        Ok(st) if st.len > MAX_CHAIN_BYTES => return Err(Refused::Limit),
        Ok(_) => {}
        Err(SourceError::NotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let text = match source.read_text(file) {
        Ok(t) => t,
        Err(SourceError::NotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if parse_tree(&text, &Bump::new()).is_err() {
        return Ok(None);
    }
    decode::<Module>(file, &text)
        .map(Some)
        .map_err(|f| Refused::Native(sentence(&f)))
}

fn read_folder(source: &dyn Source, c: &ChainAt<'_>, rule: PathRule) -> Result<Template, Refused> {
    let graph = join(c.at, GRAPH_FILE);
    if source.stat(&graph)?.len > MAX_CHAIN_BYTES {
        return Err(Refused::Limit);
    }
    let text = source.read_text(&graph)?;
    // A graph that does not parse refuses here, as `wsData` throws; one that parses and is not a graph has no steps.
    if parse_tree(&text, &Bump::new()).is_err() {
        return Err(Refused::Native(format!("expected a value in {graph}")));
    }
    let Ok(segments) = decode_graph(&graph, &text) else {
        return Ok(Template::default());
    };
    let bypassed: Vec<&str> = segments.bypassed_of().collect();
    let mut names: IndexSet<&str> = IndexSet::new();
    let derived = derive_chains(&segments, &|_| false);
    for chain in derived.values() {
        for s in &chain.use_steps {
            if s.module != CHAIN_IN && s.module != CHAIN_OUT {
                names.insert(s.module.as_str());
            }
        }
        if names.len() > MAX_STEPS {
            return Err(Refused::Limit);
        }
    }
    let mut out = Template::default();
    for name in names {
        // A node name is a file name: one that leaves `nodes/` is not read natively.
        if name.contains(['/', '\\', '\0']) || name == ".." || name == "." {
            return Err(Refused::Native(format!("node name {name} is not a file name")));
        }
        let file = join(&join(c.at, NODES_DIR), &format!("{name}.k7x"));
        let Some(module) = node_module(source, &file)? else {
            out.missing.push(format!("{name} (no {NODES_DIR}/{name}.k7x)"));
            continue;
        };
        let preset = spell(&join(&join(c.at, NODES_DIR), &format!("{name}.vstpreset")), rule)?;
        out.steps.push(TemplateStep {
            name: name.to_owned(),
            label: name.to_owned(),
            module,
            bypass: bypassed.contains(&name),
            preset: exists(source, &preset)?.then_some(preset),
        });
    }
    Ok(out)
}
