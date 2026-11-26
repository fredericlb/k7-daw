// emit_modules.rs — the imported PLUGINS: one module per plugin in the workspace's `_modules`, a node per use past the first.
// What a reader gets wrong: a module is keyed by the plugin, never by its settings, and a plugin the scan lacks gets NO module.

use std::collections::HashSet;

use k7x::edit::Value;

use crate::emit_ctx::{
    DocLayout, EmitCtx, InsertDef, ModuleDef, number, obj, slugify, text, uniquify,
};
use crate::model::PluginOut;

const NOT_SUPPORTED_REF: &str = "@meta/not-supported";
const NOT_SUPPORTED_TYPE: &str = "not_supported";

fn role_rank(role: Option<&str>) -> u8 {
    match role {
        Some("instrument") => 0,
        Some("noteFX") => 1,
        _ => 2,
    }
}

/// Instruments and note FX before audio FX; stable within.
#[must_use]
pub fn order_plugins(plugins: &[PluginOut]) -> Vec<PluginOut> {
    let mut out = plugins.to_vec();
    out.sort_by_key(|p| role_rank(p.role.as_deref()));
    out
}

/// A plugin's type folder from its role, writing `transformers` rather than the legacy `midi`.
fn type_folder(role: Option<&str>) -> &'static str {
    match role {
        Some("instrument") => "instruments",
        Some("noteFX") => "transformers",
        _ => "audio",
    }
}

fn params_value(params: &[(String, f64)]) -> Option<Value> {
    (!params.is_empty()).then(|| {
        Value::object(
            params
                .iter()
                .map(|(k, v)| (k.clone(), number(*v)))
                .collect(),
        )
    })
}

/// One insert of `p` on the chain `owner`, its module made on first use; answers the insert's node name.
pub fn module_ref_for(ctx: &mut EmitCtx<'_>, p: &PluginOut, owner: &str) -> String {
    let folder = type_folder(p.role.as_deref());
    // Text params capture state when complete or when there is no preset; else the .vstpreset is kept.
    let use_params = !p.params.is_empty() && (p.params_complete || p.path.is_none());
    let params = if use_params {
        p.params.clone()
    } else {
        Vec::new()
    };
    let preset = if use_params {
        None
    } else {
        p.path.clone().filter(|s| !s.is_empty())
    };
    let key = format!("{folder}/{}/{}", p.kind, p.name);
    let at = match ctx.module_of.get(&key) {
        Some(i) => *i,
        None => {
            let base = slugify(if p.name.is_empty() { &p.kind } else { &p.name });
            let name = uniquify(&base, &mut ctx.used_module_names);
            ctx.modules.push(ModuleDef {
                name,
                folder,
                plugin_type: p.kind.clone(),
                plugin_name: p.name.clone(),
                inserts: Vec::new(),
            });
            ctx.module_of.insert(key, ctx.modules.len() - 1);
            ctx.modules.len() - 1
        }
    };
    let name = ctx.modules[at].name.clone();
    // A sole use folds back into module.k7x, so the module's own name is taken as far as a node is concerned.
    ctx.used_node_names.insert(name.to_lowercase());
    let node = uniquify(&format!("{name}_{owner}"), &mut ctx.used_node_names);
    ctx.modules[at].inserts.push(InsertDef {
        node: node.clone(),
        params,
        preset,
    });
    node
}

/// Drops the inserts no SURVIVING chain names, so an orphan never counts towards "used more than once".
pub fn drop_orphan_inserts(ctx: &mut EmitCtx<'_>) {
    let live: HashSet<&str> = ctx
        .chains
        .values()
        .flat_map(|d| d.uses.iter().map(|s| s.module.as_str()))
        .collect();
    for m in &mut ctx.modules {
        m.inserts.retain(|i| live.contains(i.node.as_str()));
    }
}

/// A plugin used in exactly ONE place needs no node: its state folds into module.k7x.
pub fn collapse_sole_inserts(ctx: &mut EmitCtx<'_>) {
    for m in &mut ctx.modules {
        if m.inserts.len() != 1 {
            continue;
        }
        // Unless its bare name is a CHAIN's: a lane wins a bare name, so the step would name ITSELF.
        if ctx.chains.contains_key(&m.name) {
            continue;
        }
        let from = std::mem::take(&mut m.inserts[0].node);
        for d in ctx.chains.values_mut() {
            for step in &mut d.uses {
                if step.module == from {
                    step.module.clone_from(&m.name);
                }
            }
        }
    }
}

/// What writing the modules counted.
#[derive(Default)]
pub struct ModuleStats {
    pub module_nodes: usize,
    pub node_files: usize,
    pub module_preset_files: usize,
    pub missing_plugins: Vec<String>,
}

/// A missing plugin gets NO module of its own: every use becomes a node on the built-in placeholder.
fn write_missing(ctx: &mut EmitCtx<'_>, at: usize) -> usize {
    let (name, plugin_name, plugin_type) = {
        let b = &ctx.modules[at];
        (b.name.clone(), b.plugin_name.clone(), b.plugin_type.clone())
    };
    let inserts: Vec<(String, Option<Value>)> = ctx.modules[at]
        .inserts
        .iter()
        .map(|i| (i.node.clone(), params_value(&i.params)))
        .collect();
    let count = inserts.len();
    for (node, params) in inserts {
        let mut entries = vec![
            ("extends", text(NOT_SUPPORTED_REF)),
            (
                "plugin",
                obj(vec![
                    ("type", text(NOT_SUPPORTED_TYPE)),
                    ("name", text(&plugin_name)),
                ]),
            ),
        ];
        entries.extend(params.map(|p| ("params", p)));
        let note = format!(
            "MISSING PLUGIN: \"{plugin_name}\" ({plugin_type}) is not in the k7 plugin cache.\n\
             This node stands where it was: routing only, and bypassed. Install and scan it\n\
             ('k7 engine scan') and re-import, or point this node's `extends` at a real module.\n\
             The params below are the ones the export carried, kept so nothing is lost."
        );
        let file = if node.is_empty() { &name } else { &node };
        ctx.write_doc(
            &format!("nodes/{file}.k7x"),
            &obj(entries),
            &DocLayout {
                note: Some(&note),
                ..DocLayout::default()
            },
        );
    }
    count
}

/// Modules to module.k7x, plus one flat nodes/<node>.k7x per insert past the first.
pub fn write_modules(ctx: &mut EmitCtx<'_>, scan: Option<&HashSet<String>>) -> ModuleStats {
    let verify = scan.is_some_and(|s| !s.is_empty());
    let mut stats = ModuleStats::default();
    for at in 0..ctx.modules.len() {
        if ctx.modules[at].inserts.is_empty() {
            continue;
        }
        let known = !verify
            || scan.is_some_and(|s| s.contains(&ctx.modules[at].plugin_name.to_lowercase()));
        if !known {
            let name = ctx.modules[at].plugin_name.clone();
            if !stats.missing_plugins.contains(&name) {
                stats.missing_plugins.push(name);
            }
            stats.node_files += write_missing(ctx, at);
            continue;
        }
        write_module(ctx, at, &mut stats);
    }
    stats
}

fn write_module(ctx: &mut EmitCtx<'_>, at: usize, stats: &mut ModuleStats) {
    let (name, folder, plugin_type, plugin_name, count) = {
        let b = &ctx.modules[at];
        (
            b.name.clone(),
            b.folder,
            b.plugin_type.clone(),
            b.plugin_name.clone(),
            b.inserts.len(),
        )
    };
    let dir = format!("{folder}/{name}");
    let sole = count == 1 && ctx.modules[at].inserts[0].node.is_empty();
    let mut entries = vec![(
        "plugin",
        obj(vec![
            ("type", text(&plugin_type)),
            ("name", text(&plugin_name)),
        ]),
    )];
    if sole {
        entries.extend(params_value(&ctx.modules[at].inserts[0].params).map(|p| ("params", p)));
    }
    let note = (!sole).then(|| {
        format!(
            "{plugin_name} — used {count} times in this song, so each use is a\n\
             NODE in nodes/ that extends this module. The plugin is declared once here;\n\
             each node carries only its own settings."
        )
    });
    ctx.write_shared_doc(
        &format!("{dir}/module.k7x"),
        &obj(entries),
        &DocLayout {
            note: note.as_deref(),
            ..DocLayout::default()
        },
    );
    stats.module_nodes += 1;
    for i in 0..count {
        let (node, params, preset) = {
            let ins = &ctx.modules[at].inserts[i];
            (
                ins.node.clone(),
                params_value(&ins.params),
                ins.preset.clone(),
            )
        };
        if !node.is_empty() {
            let mut e = vec![("extends", text(&format!("@workspace/{folder}/{name}")))];
            e.extend(params.map(|p| ("params", p)));
            let note =
                format!("{node} — inherits {name} (the plugin); only what this use sets differs.");
            ctx.write_doc(
                &format!("nodes/{node}.k7x"),
                &obj(e),
                &DocLayout {
                    note: Some(&note),
                    ..DocLayout::default()
                },
            );
            stats.node_files += 1;
        }
        let bytes = preset.and_then(|p| ctx.assets.get(p.as_str()).map(|b| (*b).clone()));
        if let Some(bytes) = bytes {
            // Beside the file whose state it is: the node's in the project, or the module's folder for a sole use.
            let path = if node.is_empty() {
                format!("_modules/{dir}/{name}.vstpreset")
            } else {
                format!("{}/nodes/{node}.vstpreset", ctx.project)
            };
            ctx.write_binary(path, bytes);
            stats.module_preset_files += 1;
        }
    }
}
