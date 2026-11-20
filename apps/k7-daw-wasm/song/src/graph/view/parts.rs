// graph/view/parts.rs — the pieces off the signal path: curves, assemblies, a note's text, and the tap a live input listens at.
// A reader would get wrong that EVERY curve of the project travels, not only the bound ones: the curve editor lists them all (428).

use super::web::{WebAssemblyParam, WebChain, WebCurve, WebDocNote, WebNodeAssembly, WebPlug};
use crate::assemblies::{Assembly, AssemblyPort};
use crate::curves::CurveFile;
use crate::graph::{GraphDoc, Spend};
use crate::lanes::live::input_landing_track;
use crate::modules::{IoKind, ModuleKind};
use indexmap::IndexMap;
use song_schema::CurveSpec;
use std::collections::HashMap;
use std::sync::Arc;

/// A curve as the Chains tab labels its node: its type, an oscillator's shape, a drawn curve's points, the whole definition.
fn web_curve(c: &Arc<CurveFile>) -> WebCurve {
    let (kind, shape, points) = match &c.spec {
        CurveSpec::Lfo(s) => ("lfo", Some(format!("{:?}", s.shape).to_lowercase()), None),
        CurveSpec::Custom(s) => ("custom", None, Some(s.points.clone())),
        CurveSpec::Onoff(s) => ("onoff", None, Some(s.points.clone())),
        CurveSpec::Adsr(_) => ("adsr", None, None),
    };
    WebCurve {
        kind: kind.to_owned(),
        shape,
        points,
        spec: (**c).clone(),
    }
}

/// Every curve the project has, in file order, each paid for by the size of its definition twice (the points are held twice).
pub(super) fn curves(all: &IndexMap<String, Arc<CurveFile>>, spend: &mut Spend) -> Option<IndexMap<String, WebCurve>> {
    let mut out = IndexMap::new();
    for (id, c) in all {
        let w = web_curve(c);
        let bytes = serde_json::to_vec(&w).map_or(0, |v| v.len()).saturating_add(id.len());
        if !spend.bytes(bytes) {
            break;
        }
        out.insert(id.clone(), w);
    }
    (!out.is_empty()).then_some(out)
}

/// A note's TEXT, keyed by DOC modules and nothing else.
pub(super) fn doc_notes(g: &GraphDoc<'_>, spend: &mut Spend) -> IndexMap<String, WebDocNote> {
    let mut out = IndexMap::new();
    for (name, m) in &g.modules {
        if m.kind != ModuleKind::Doc {
            continue;
        }
        let content = m.m.authored.content.clone().unwrap_or_default();
        if !spend.bytes(name.len() + content.len()) {
            break;
        }
        out.insert((*name).to_owned(), WebDocNote { content });
    }
    out
}

fn plug(p: &AssemblyPort) -> WebPlug {
    WebPlug {
        name: p.name.clone(),
        kind: p.kind,
        node: p.node.clone(),
        port: p.port.clone(),
        mandatory: p.mandatory,
    }
}

/// The boxes, with the engine address of each promoted param, which only the built steps know.
pub(super) fn assemblies(
    all: &IndexMap<String, Assembly>,
    chains: &IndexMap<String, WebChain>,
    spend: &mut Spend,
) -> Option<IndexMap<String, WebNodeAssembly>> {
    let mut ids: HashMap<&str, &str> = HashMap::new();
    for c in chains.values() {
        for s in &c.use_steps {
            if let Some(id) = &s.plugin_id {
                ids.insert(&s.module, id);
            }
        }
    }
    let mut out = IndexMap::new();
    for (id, a) in all {
        let params: IndexMap<String, WebAssemblyParam> = a
            .params
            .iter()
            .map(|(name, t)| {
                (
                    name.clone(),
                    WebAssemblyParam {
                        node: t.node.0.clone(),
                        param: t.param.0.clone(),
                        plugin_id: ids.get(t.node.0.as_str()).map(|s| (*s).to_owned()),
                    },
                )
            })
            .collect();
        let w = WebNodeAssembly {
            uses: a.uses.clone(),
            label: a.label.clone(),
            members: a.members.values().cloned().collect(),
            inputs: (!a.inputs.is_empty()).then(|| a.inputs.iter().map(plug).collect()),
            outputs: (!a.outputs.is_empty()).then(|| a.outputs.iter().map(plug).collect()),
            params: (!params.is_empty()).then_some(params),
            missing: a.missing,
        };
        let bytes = serde_json::to_vec(&w).map_or(0, |v| v.len()).saturating_add(id.len());
        if !spend.bytes(bytes) {
            break;
        }
        out.insert(id.clone(), w);
    }
    (!out.is_empty()).then_some(out)
}

/// Where each live input's waveform listens: a second pass over the built steps (the first step with a plugin id on its lane's chain).
pub(super) fn input_taps(g: &GraphDoc<'_>, chains: &mut IndexMap<String, WebChain>, spend: &mut Spend) {
    let is_track = |id: &str| g.tracks.contains_key(id);
    let mut first: HashMap<String, Option<String>> = HashMap::new();
    for (name, c) in &g.chains {
        let Some(track) = input_landing_track(c, &is_track) else {
            continue;
        };
        let id = first
            .entry(track.to_owned())
            .or_insert_with(|| {
                chains
                    .get(track)
                    .and_then(|w| w.use_steps.iter().find_map(|s| s.plugin_id.clone()))
            })
            .clone();
        let Some(id) = id else { continue };
        let Some(web) = chains.get_mut(name) else { continue };
        for s in &mut web.use_steps {
            let is_in = g
                .modules
                .get(s.module.as_str())
                .is_some_and(|m| m.io == Some(IoKind::In));
            if is_in && spend.bytes(id.len()) {
                s.tap = Some(super::web::WebTap {
                    id: id.clone(),
                    side: "input".to_owned(),
                });
            }
        }
    }
}
