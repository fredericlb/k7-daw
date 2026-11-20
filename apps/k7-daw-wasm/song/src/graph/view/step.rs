// graph/view/step.rs — one step of a chain, built ONCE per module name and cloned per use (421).
// A reader would get wrong that the body knows nothing of the use: `pluginId`, `enabled` and `tap` are set by the caller, per use.

use super::choices::{Walk, Walks, fill};
use super::flags::{PluginFlags, flags_of, param_defaults, shown_ui, stage_of, takes_stage};
use super::sounds::{kit_sounds, numeric_channels, rack_sounds};
use super::web::{Params, WebAutomation, WebChainStep, WebModelDownload, WebPortIn, WebPortOut};
use crate::curves::CurveFile;
use crate::graph::{GraphModule, NodePlace, Spend};
use crate::modules::files::FILE_PARAMS;
use crate::modules::{IoKind, ModuleKind, Scan};
use crate::resolve::{MIXER_PLUGIN, alternatives_of, current_alternative, current_engine, engine_names, resolved_cc};
use indexmap::IndexMap;
use std::sync::Arc;

use super::flags::{INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM};
use crate::modules::ParamValue;

/// What a body is built from, besides the module.
pub(super) struct Facts<'a> {
    pub scan: &'a Scan,
    pub curves: &'a IndexMap<String, Arc<CurveFile>>,
}

/// A model the module names and the machine has not got, with where to fetch it: the right-click's "Download model".
pub fn missing_model(m: &GraphModule<'_>) -> Option<(String, Option<String>)> {
    let assets = m.m.assets.as_ref()?;
    for field in FILE_PARAMS {
        let Some(file) = assets.get(field).filter(|f| !f.is_empty()) else {
            continue;
        };
        let have = match field {
            "model" => m.m.model.as_deref(),
            _ => m.m.onnx.as_deref(),
        };
        if have.is_some_and(|h| !h.is_empty()) {
            continue;
        }
        let url =
            m.m.authored
                .model_url
                .as_ref()
                .map(|u| u.0.clone())
                .filter(|u| !u.is_empty());
        return Some((file.clone(), url));
    }
    None
}

fn params_of(m: &GraphModule<'_>) -> Params {
    let a = &m.m.authored;
    let mut p: Params = a.params.clone().unwrap_or_default();
    // The engine's blend and levels under their intrinsic names, so a MIX knob draws what the file says.
    if let Some(v) = &a.mix {
        p.insert(MIX_PARAM.to_owned(), ParamValue::Number(v.0));
    }
    if let Some(v) = &a.input_volume {
        p.insert(INPUT_VOLUME_PARAM.to_owned(), ParamValue::Number(v.0));
    }
    if let Some(v) = &a.output_volume {
        p.insert(OUTPUT_VOLUME_PARAM.to_owned(), ParamValue::Number(v.0));
    }
    p
}

fn automations_of(m: &GraphModule<'_>, curves: &IndexMap<String, Arc<CurveFile>>) -> Option<Vec<WebAutomation>> {
    let out: Vec<WebAutomation> = m
        .automations
        .iter()
        .filter(|(_, curve, _)| curves.contains_key(*curve))
        .map(|(param, curve, trigger)| WebAutomation {
            param: (*param).to_owned(),
            curve: (*curve).to_owned(),
            trigger: trigger.map(str::to_owned),
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

/// The step of module `name`, without what a use adds; none when the budget stops a walk it needs.
pub(super) fn body(
    facts: &Facts<'_>,
    name: &str,
    m: &GraphModule<'_>,
    place: Option<&NodePlace<'_>>,
    walks: &mut Walks<'_>,
    spend: &mut Spend,
    file: &str,
) -> Walk<WebChainStep> {
    let flags: PluginFlags = flags_of(m, facts.scan);
    let a = &m.m.authored;
    let ui = shown_ui(m);
    let mut s = WebChainStep::new(name);
    s.uses = m.extends.map(str::to_owned);
    s.place = place.map(|p| p.place.clone());
    if takes_stage(m) {
        let stage = stage_of(m);
        s.stage = (!stage.is_empty()).then_some(stage);
    }
    s.inputs = (m.inputs.len() > 1).then(|| {
        m.inputs
            .iter()
            .map(|i| WebPortIn {
                name: i.name.clone(),
                kind: i.kind,
                mandatory: (!i.mandatory).then_some(false),
            })
            .collect()
    });
    s.outputs = (m.outputs.len() > 1).then(|| {
        m.outputs
            .iter()
            .map(|o| WebPortOut {
                name: o.name.clone(),
                kind: o.kind,
            })
            .collect()
    });
    s.assets = m.m.assets.clone();
    s.base = a.base.clone().filter(|b| !b.is_empty());
    s.shader = a.shader.as_ref().map(|x| x.0.clone()).filter(|x| !x.is_empty());
    s.events = (m.category == Some("instruments")).then(|| vec!["note-on".to_owned()]);
    s.category = m.category.map(str::to_owned);
    s.emits = m.emits;
    s.stub = flags.stub;
    s.not_supported = flags.not_supported.then_some(true);
    s.unknown_plugin = flags.unknown.then_some(true);
    s.has_native_plugin_ui = flags.native_ui;
    s.strip = (a.plugin.as_ref().and_then(|p| p.name.as_deref()) == Some(MIXER_PLUGIN)).then_some(true);
    s.model_download = missing_model(m).and_then(|(file, url)| url.map(|url| WebModelDownload { file, url }));
    s.cc = resolved_cc(Some(m.m));
    s.params = params_of(m);
    s.defaults = ui.as_ref().and_then(|u| param_defaults(m, u));
    let s_ui = ui.clone();
    s.ui = ui;
    let alts = alternatives_of(a);
    if alts.len() >= 2 {
        s.alternative = alts.get(current_alternative(a)).map(|x| x.name.clone());
        s.alternatives = Some(alts.into_iter().map(|x| x.name).collect());
    }
    let engines = engine_names(m.m);
    if engines.len() >= 2 {
        s.engine = current_engine(m.m);
        s.engines = Some(engines);
    }
    if m.kind == ModuleKind::Io {
        match m.io {
            Some(IoKind::In) => {
                s.monitor = Some(m.monitor.name().to_owned());
                s.channel = numeric_channels(m);
            }
            Some(IoKind::Camera) => s.camera = Some(a.camera.clone().unwrap_or_default()),
            Some(IoKind::Midi) => s.device = Some(a.device.clone().unwrap_or_default()),
            _ => {}
        }
    }
    s.automations = automations_of(m, facts.curves);
    let rack = rack_sounds(m);
    let kit = rack.is_none().then(|| kit_sounds(m)).flatten();
    s.rack_kit = kit.is_some().then_some(true);
    s.rack = rack.or(kit);
    Ok(fill(walks, &mut s, m, s_ui.as_ref(), file, spend)?.map(|()| s))
}

/// The id the engine addresses a plugin by, only when the engine will really have one.
pub(super) fn plugin_id(chain: &str, name: &str, m: &GraphModule<'_>, scan: &Scan) -> Option<String> {
    let f = flags_of(m, scan);
    (f.slot && !f.stub && !f.not_supported && !f.unknown && !f.offline).then(|| format!("{chain}:{name}"))
}

/// The bytes a body serializes to: the weight of a copy.
pub(super) fn serialized_len(s: &WebChainStep) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(b.len());
            Ok(b.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut c = Count(0);
    // A value that cannot be written has no weight; serde_json writes every field of a step.
    let _ = serde_json::to_writer(&mut c, s);
    c.0
}
