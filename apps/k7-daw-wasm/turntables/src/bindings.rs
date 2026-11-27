// bindings.rs — join the project's `views/mix.k7x` to the desk and to the graph: which control drives which param, over which range.
// A reader would get wrong where the join lives: the graph half is `chains::bindings`, shared with the mastering strip; this is the desk's.

use crate::controls::{MixControl, mix_controls};
use crate::vocab::{DECKS, DeckTempos};
use chains::bindings::{BindGraph, BindingProblem, PluglessPolicy, ResolvedTarget, SurfaceBinding, resolve_mapping};
use chains::widget_value::WidgetScale;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// Must name a module: the ones driving a PARAM, minus `cue`, `echo` and the stems, which a complete desk may leave out.
const WIRED_PER_DECK: [&str; 7] = ["tempo", "eqLow", "eqMid", "eqHigh", "filter", "trim", "level"];

/// What a binding addresses: the module and param, the plugin or the track that moves it, the widget that states its range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct Target {
    pub module: String,
    pub param: String,
    #[serde(default, rename = "pluginId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "pluginId"))]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").PageWidget"))]
    pub widget: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

impl From<ResolvedTarget> for Target {
    fn from(t: ResolvedTarget) -> Target {
        Target {
            module: t.module,
            param: t.param,
            plugin_id: t.plugin_id,
            track: t.track,
            widget: t.widget,
            invert: t.invert,
        }
    }
}

/// A binding that resolved to something that can be moved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct DeskBinding {
    pub control: MixControl,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub target: Target,
    /// A second param this control drives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub also: Option<Target>,
}

/// Every binding of a mapping that resolved, and a sentence for each that did not.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeskBindings {
    pub bindings: Vec<DeskBinding>,
    pub problems: Vec<BindingProblem>,
}

/// What an `offline` tempo module should read, for a host to lend to the graph.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Published {
    pub module: String,
    pub param: String,
    pub value: f64,
}

/// A mapping as the page reads it from `views/mix.k7x`: a line per control, or null for one left unbound.
pub type Mapping = IndexMap<String, Option<SurfaceBinding>>;

/// Every control a turntables surface needs wired, derived from the decks so a third is not skipped.
pub fn required_controls() -> Vec<MixControl> {
    mix_controls()
        .into_iter()
        .filter(|c| {
            c.id == "crossfade"
                || DECKS
                    .iter()
                    .any(|d| WIRED_PER_DECK.iter().any(|s| c.id == format!("{d}.{s}")))
        })
        .collect()
}

/// The required controls this mapping does not NAME; the optional ones are left alone.
pub fn unmapped(mapping: &Mapping) -> Vec<MixControl> {
    let required = required_controls();
    let ids: Vec<String> = required.iter().map(|c| c.id.clone()).collect();
    let missing: HashSet<String> = chains::bindings::unmapped_controls(&ids, mapping).into_iter().collect();
    required.into_iter().filter(|c| missing.contains(&c.id)).collect()
}

/// Resolve every binding against the graph: three failures, all reported rather than dropped.
pub fn resolve_bindings(mapping: &Mapping, graph: &BindGraph) -> DeskBindings {
    let controls = mix_controls();
    let ids: IndexSet<String> = controls.iter().map(|c| c.id.clone()).collect();
    // `track` and not `refuse`: an `offline` module compiles to no plugin, and a tempo fader binds it by its track.
    let done = resolve_mapping(mapping, graph, &ids, "of the desk", PluglessPolicy::Track, true);
    let bindings = done
        .rows
        .into_iter()
        .filter_map(|r| {
            let control = controls.iter().find(|c| c.id == r.control)?.clone();
            Some(DeskBinding {
                control,
                target: r.target.into(),
                also: r.also.map(Target::from),
            })
        })
        .collect();
    DeskBindings {
        bindings,
        problems: done.problems,
    }
}

/// Is this binding's target a plugin the engine has LOADED? The document decides, not the graph.
fn has_plugin(b: &DeskBinding, plugins: &HashSet<&str>) -> bool {
    b.target.plugin_id.as_deref().is_some_and(|id| plugins.contains(id))
}

/// The control's own deck: `deckA.tempo` is deck A's fader wherever it points.
fn deck_of(b: &DeskBinding) -> &str {
    b.control.id.split('.').next().unwrap_or("")
}

/// The stretches to apply, keyed by the TRACK each lands on: the tempo through its module.
pub fn stretch_targets(bindings: &[DeskBinding], by_deck: &DeckTempos, plugins: &[String]) -> DeckTempos {
    let loaded: HashSet<&str> = plugins.iter().map(String::as_str).collect();
    let mut out = DeckTempos::new();
    for b in bindings {
        let Some(track) = b.target.track.as_deref().filter(|t| !t.is_empty()) else {
            continue;
        };
        if has_plugin(b, &loaded) {
            continue;
        }
        if let Some(tempo) = by_deck.get(deck_of(b)) {
            out.insert(track.to_owned(), tempo.clone());
        }
    }
    out
}

/// What every tempo module with no plugin should READ, for a host to lend to the graph.
pub fn offline_publish(bindings: &[DeskBinding], by_deck: &DeckTempos, source_param: &str) -> Vec<Published> {
    let mut out = Vec::new();
    for b in bindings {
        // A real plugin: the engine reports that one itself.
        if b.target.plugin_id.as_deref().is_some_and(|p| !p.is_empty()) {
            continue;
        }
        if b.control.wants.as_deref() != Some("tempo") {
            continue;
        }
        let t = by_deck.get(deck_of(b));
        out.push(Published {
            module: b.target.module.clone(),
            param: b.target.param.clone(),
            value: t.map_or(0.0, |t| t.target),
        });
        out.push(Published {
            module: b.target.module.clone(),
            param: source_param.to_owned(),
            value: t.map_or(0.0, |t| t.natural),
        });
    }
    out
}

/// What to send for a control at `value`: normalised by the CONTROL's range, expanded by the WIDGET's.
pub fn value_for(b: &DeskBinding, value: f64) -> f64 {
    let [lo, hi] = b.control.range;
    // Mirrored first, in the control's own range: two fades driven this way are a crossfader.
    let v = if b.target.invert == Some(true) {
        lo + hi - value
    } else {
        value
    };
    let Some(w) = b.target.widget.as_ref() else { return v };
    let scale: WidgetScale = serde_json::from_value(w.clone()).unwrap_or_default();
    if scale.min.is_none() || scale.max.is_none() {
        return v;
    }
    let unit = if hi > lo {
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    scale.denorm(unit)
}
