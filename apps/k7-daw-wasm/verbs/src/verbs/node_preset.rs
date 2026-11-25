// node_preset.rs — `node preset apply|save|update` and `node engine`, after verbs/preset.ts and engine.ts.
// A reader would get wrong where a preset lives: a `presets/` tree keyed by the module it extends, outside the project.

use super::node_shared::{plugin_module, split_args};
use super::shared::Done;
use crate::host::{Host, Thrown};
use song_edit::engine_switch::{module_engines, select_module_engine};
use song_edit::node_presets::{PresetMode, apply_preset_to_node, write_node_preset};
use song_edit::presets::{WORKSPACE_SCOPE, list_node_presets};

/// Whether `module` compiles to a plugin, the one thing a values-only push can patch.
pub fn has_plugin(h: &mut Host, module: &str) -> Result<bool, Thrown> {
    let p = h.project()?;
    Ok(p.tracks
        .iter()
        .flat_map(|t| t.plugins.iter())
        .chain(p.master_plugins.iter())
        .any(|pl| plugin_module(&pl.id) == module))
}

/// `node preset apply <node> <scope> "<name>"`; bare, the node's presets are listed.
pub fn preset_apply(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let Some(node) = w.first().cloned() else {
        h.refuse([r#"usage: node preset apply <node> <scope> "<name>""#.to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let (Some(scope), Some(name)) = (w.get(1).cloned(), w.get(2).cloned()) else {
        match list_node_presets(&*h.fs, &h.env, &dir, &node) {
            Ok(l) => {
                let line = match &l.reference {
                    None => format!("{node} extends no module, so it has no presets"),
                    Some(r) if l.presets.is_empty() => {
                        format!("{node} ({r}) has no presets — 'node preset save {node} <name>' keeps one")
                    }
                    Some(r) => {
                        let all: Vec<String> = l.presets.iter().map(|p| format!("{}:{}", p.scope, p.name)).collect();
                        format!("{node} ({r}): {}", all.join("  "))
                    }
                };
                h.log([line]);
            }
            Err(e) => h.refuse([format!("node preset: {e}")]),
        }
        return Ok(());
    };
    let done = match apply_preset_to_node(h.fs, &h.env, &dir, &node, &scope, &name) {
        Ok(d) => d,
        Err(e) => {
            h.refuse([format!("node preset: {e}")]);
            return Ok(());
        }
    };
    let engine = done.missing_engine.as_ref().map_or(String::new(), |e| {
        format!(" — its engine '{e}' is not here, applied onto the current one")
    });
    let base = done.missing_base.as_ref().map_or(String::new(), |b| {
        format!(" — its base '{b}' is not here, applied onto the current one")
    });
    h.log([format!("{node}: preset '{name}' ({scope}){engine}{base}")]);
    // Another engine is another plugin, a base another state and a kit is built with its sampler: those reload.
    if !(done.switched || done.rebased || done.kit) && has_plugin(h, &node)? {
        h.apply_project();
    } else {
        h.reload();
    }
    Ok(())
}

/// `node preset save <node> "<name>" [workspace|<library>]` — a new preset file, no project touched.
pub fn preset_save(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (Some(node), Some(name)) = (w.first().cloned(), w.get(1).cloned()) else {
        h.refuse([r#"usage: node preset save <node> "<name>" [workspace|<library>]"#.to_owned()]);
        return Ok(());
    };
    let scope = w.get(2).cloned().unwrap_or_else(|| WORKSPACE_SCOPE.to_owned());
    let dir = h.song_path.clone();
    match write_node_preset(h.fs, &h.env, &dir, &node, &scope, &name, PresetMode::New) {
        Ok((_, r, _)) => h.log([format!("{node}: saved preset '{name}' for {r} in {scope}")]),
        Err(e) => h.refuse([format!("node preset save: {e}")]),
    }
    Ok(())
}

/// `node preset update <node> <scope> "<name>"` — the preset rewritten from the node; undo lays the old file back.
pub fn preset_update(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (Some(node), Some(scope), Some(name)) = (w.first().cloned(), w.get(1).cloned(), w.get(2).cloned()) else {
        h.refuse([r#"usage: node preset update <node> <scope> "<name>""#.to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match write_node_preset(h.fs, &h.env, &dir, &node, &scope, &name, PresetMode::Update) {
        Ok(_) => h.log([format!("{node}: updated preset '{name}' in {scope}")]),
        Err(e) => h.refuse([format!("node preset update: {e}")]),
    }
    Ok(())
}

/// `node engine <node> [name]` — another of the module's engines, always a reload; bare, the engines are listed.
pub fn node_engine(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let Some(module) = w.first().cloned() else {
        h.refuse(["usage: node engine <node> [name]".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let Some(name) = w.get(1).cloned() else {
        match module_engines(&*h.fs, &h.env, &dir, &module) {
            Ok((names, current)) if !names.is_empty() => {
                let all: Vec<String> = names
                    .iter()
                    .map(|n| {
                        if Some(n) == current.as_ref() {
                            format!("[{n}]")
                        } else {
                            n.clone()
                        }
                    })
                    .collect();
                h.log([format!("{module}: {}", all.join("  "))]);
            }
            Ok(_) => h.log([format!("{module} has no engines")]),
            Err(e) => h.refuse([format!("node engine: {e}")]),
        }
        return Ok(());
    };
    match select_module_engine(h.fs, &h.env, &dir, &module, &name) {
        Ok(_) => {
            h.log([format!("{module}: running engine '{name}'")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node engine: {e}")]),
    }
    Ok(())
}
