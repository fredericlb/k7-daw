// modules/merge.rs — a parent module merged UNDER a child: the child's defined values win, four entries merge one by one.
// A reader would get wrong that `params` is always present afterwards (`{}` at least), as the TypeScript's merge leaves it.

use super::LoadedModule;
use super::rack::Rack;
use super::schema::{EngineSet, Module};
use super::ui::Ui;
use super::unions::Keyed;

macro_rules! fill {
    ($out:ident, $parent:ident, $($f:ident),*) => {
        $( if $out.$f.is_none() { $out.$f = $parent.$f.clone(); } )*
    };
}

fn merge_rack(p: &Rack, c: &Rack) -> Rack {
    let mut symbols = p.symbols.clone();
    symbols.extend(c.symbols.clone());
    Rack {
        rack_type: c.rack_type,
        source: c.source.clone().or_else(|| p.source.clone()),
        symbols,
    }
}

fn merge_ui(p: &Ui, c: &Ui) -> Ui {
    let mut widgets = p.widgets.clone();
    widgets.extend(c.widgets.clone());
    Ui {
        cols: c.cols,
        rows: c.rows,
        widgets,
        tabs: c.tabs.clone().or_else(|| p.tabs.clone()),
        vertical: c.vertical.clone().or_else(|| p.vertical.clone()),
        script: c.script.clone().or_else(|| p.script.clone()),
    }
}

/// Per engine, then per param inside it, so a node setting one knob of one engine keeps the rest.
fn merge_engines(
    p: Option<&Keyed<super::scalars::EngineKey, EngineSet>>,
    c: Option<&Keyed<super::scalars::EngineKey, EngineSet>>,
) -> Option<Keyed<super::scalars::EngineKey, EngineSet>> {
    let (Some(p), Some(c)) = (p, c) else {
        return c.or(p).cloned();
    };
    let mut out = p.0.clone();
    for (name, set) in &c.0 {
        let mut params = p.0.get(name).and_then(|e| e.params.clone()).unwrap_or_default();
        params.extend(set.params.clone().unwrap_or_default());
        out.insert(name.clone(), EngineSet { params: Some(params) });
    }
    Some(Keyed::new(out))
}

/// `parent` under `child`; an undefined child value is not a value.
pub fn merge_authored(parent: &Module, child: &Module) -> Module {
    let mut out = child.clone();
    fill!(
        out,
        parent,
        extends,
        alternatives,
        alternative,
        status,
        plugin,
        io,
        inputs,
        outputs,
        module_type,
        shader,
        shape,
        font,
        align,
        background,
        shadow,
        tap_outs,
        kit,
        cold,
        model_url,
        channel,
        camera,
        device,
        frame,
        source,
        monitor,
        gain,
        mix,
        input_volume,
        output_volume,
        base,
        automations,
        content,
        events,
        color,
        disabled,
        tracks,
        strudel,
        video_source,
        lut,
        frozen,
        engine,
        plugin_at
    );
    let mut params = parent.params.clone().unwrap_or_default();
    params.extend(child.params.clone().unwrap_or_default());
    out.params = Some(params);
    out.rack = match (&parent.rack, &child.rack) {
        (Some(p), Some(c)) => Some(merge_rack(p, c)),
        (p, c) => c.clone().or_else(|| p.clone()),
    };
    out.ui = match (&parent.ui, &child.ui) {
        (Some(p), Some(c)) => Some(merge_ui(p, c)),
        (p, c) => c.clone().or_else(|| p.clone()),
    };
    out.engines = merge_engines(parent.engines.as_ref(), child.engines.as_ref());
    out
}

/// The loader's fields, merged as the TypeScript's `OWN_ONLY` does by key presence: a module's identity is never inherited.
pub fn merge_loaded(parent: &LoadedModule, child: &LoadedModule) -> LoadedModule {
    let assets = match (&parent.assets, &child.assets) {
        (None, None) => None,
        (p, c) => {
            let mut all = p.clone().unwrap_or_default();
            all.extend(c.clone().unwrap_or_default());
            Some(all)
        }
    };
    macro_rules! own_or_parent {
        ($($f:ident),*) => { LoadedModule {
            authored: merge_authored(&parent.authored, &child.authored),
            file: child.file.clone(),
            node: child.node.clone(),
            name: child.name.clone(),
            home: child.home.clone(),
            chain: child.chain.clone(),
            node_id: child.node_id.clone(),
            preset_rel: child.preset_rel.clone(),
            assets,
            kind: child.kind.or(parent.kind),
            generated: child.generated.or(parent.generated),
            picture: child.picture.or(parent.picture),
            $( $f: child.$f.clone().or_else(|| parent.$f.clone()), )*
        } };
    }
    own_or_parent!(
        category,
        engine_refs,
        engine_of,
        jsdsp,
        faust,
        folder_abs,
        preset,
        model,
        onnx,
        ir,
        cc
    )
}
