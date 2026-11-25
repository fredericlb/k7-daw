// node_state.rs — one node's state: bypass, alternatives, freeze and its external UI, after verbs/node.ts.
// A reader would get wrong freeze: only the unfreeze half writes here, the bounce being the host's `render` (docs/verbs-port.md).

use super::node_preset::has_plugin;
use super::node_shared::{chain_of_module, plugin_module, split_args};
use super::shared::{Done, join, split_words};
use crate::host::Host;
use song::doc::ProjectDoc;
use song::modules::files::is_external_format;
use song_edit::alternative_switch::select_module_alternative;
use song_edit::confine::confine_to;
use song_edit::graph_edit::set_module_bypass;
use song_edit::loader::resolve_node;
use song_edit::project_edit::{module_alternatives, new_module_alternative, set_module_frozen};

/// Each compiled plugin with the lane that plays it, the master run's under `@master`.
fn plugins(p: &ProjectDoc) -> Vec<(&str, &str, &str)> {
    let lanes = p
        .tracks
        .iter()
        .flat_map(|t| t.plugins.iter().map(move |pl| (t.id.as_str(), pl)));
    lanes
        .chain(p.master_plugins.iter().map(|pl| (song::lanes::MASTER_OWNER, pl)))
        .map(|(owner, pl)| (owner, pl.id.as_str(), pl.plugin_type.as_str()))
        .collect()
}

/// `node external-ui <node>` — the window of one external plugin, asked of the host as `showNativePluginUI`.
pub fn external_ui(a: &str, h: &mut Host) -> Done {
    let name = a.trim().to_owned();
    let p = h.project()?;
    let modules: Vec<(&str, &str, &str)> = plugins(&p)
        .into_iter()
        .filter(|(_, _, t)| is_external_format(&t.to_lowercase()))
        .collect();
    let names: Vec<&str> = modules.iter().map(|(_, id, _)| plugin_module(id)).collect();
    if name.is_empty() {
        h.refuse([if names.is_empty() {
            "no external plugins in this project".to_owned()
        } else {
            format!("open which? {}", names.join(", "))
        }]);
        return Ok(());
    }
    let hit = modules.iter().find(|(_, id, _)| plugin_module(id) == name).or_else(|| {
        modules
            .iter()
            .find(|(_, id, _)| plugin_module(id).eq_ignore_ascii_case(&name))
    });
    let Some((track, id, kind)) = hit else {
        let hint = if names.is_empty() {
            String::new()
        } else {
            format!(" — try: {}", names.join(", "))
        };
        h.refuse([format!("no external plugin module '{name}'{hint}")]);
        return Ok(());
    };
    h.ask("showNativePluginUI", serde_json::json!([id]));
    h.log([format!("opening {} ({kind}) on {track}…", plugin_module(id))]);
    Ok(())
}

/// `node bypass <node> [on|off]` — a toggle without the word; a module with a plugin is pushed, else reloaded.
pub fn node_bypass(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let state = w.get(1).copied();
    if w.is_empty() || w.len() > 2 || state.is_some_and(|s| !matches!(s, "on" | "off" | "true" | "false")) {
        h.refuse(["usage: node bypass <node> [on|off]".to_owned()]);
        return Ok(());
    }
    let module = w[0].to_owned();
    let Some((chain, on)) = chain_of_module(h, &module) else {
        h.refuse([format!("no module \"{module}\" in any chain")]);
        return Ok(());
    };
    let bypass = state.map_or(on, |s| s == "on" || s == "true");
    let dir = h.song_path.clone();
    if let Err(e) = set_module_bypass(h.fs, &h.env, &dir, &module, bypass) {
        h.refuse([format!("bypass failed: {e}")]);
        return Ok(());
    }
    h.log([format!(
        "{chain}: {module} {}",
        if bypass { "bypassed" } else { "active" }
    )]);
    let id = format!("{chain}:{module}");
    let p = h.project()?;
    if plugins(&p).iter().any(|(_, pid, _)| *pid == id) {
        h.apply_project();
    } else {
        h.reload();
    }
    Ok(())
}

/// `node alternative select <node> [name]` — bare, the sets are listed, the running one bracketed.
pub fn alt_select(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let Some(module) = w.first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: node alternative select <node> [name]".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let (names, current) = match module_alternatives(&*h.fs, &h.env, &dir, &module) {
        Ok(x) => x,
        Err(e) => {
            h.refuse([format!("node alternative: {e}")]);
            return Ok(());
        }
    };
    let Some(name) = w.get(1).map(|s| (*s).to_owned()) else {
        h.log([if names.len() > 1 {
            let all: Vec<String> = names
                .iter()
                .map(|n| if *n == current { format!("[{n}]") } else { n.clone() })
                .collect();
            format!("{module}: {}", all.join("  "))
        } else {
            format!("{module} has one set of settings — 'node alternative create {module}' starts a second")
        }]);
        return Ok(());
    };
    let before = h.project()?;
    let values = match select_module_alternative(h.fs, &h.env, &dir, &module, &name) {
        Ok((_, v)) => v,
        Err(e) => {
            h.refuse([format!("node alternative: {e}")]);
            return Ok(());
        }
    };
    h.log([format!("{module}: running alternative '{name}'")]);
    // Numbers alone are a diff of the held doc, unless a whole set drops a param a diff would leave behind.
    let diff = values.is_some_and(|v| {
        let mine: Vec<_> = before
            .tracks
            .iter()
            .flat_map(|t| t.plugins.iter())
            .chain(before.master_plugins.iter())
            .filter(|pl| plugin_module(&pl.id) == module)
            .collect();
        !mine.is_empty()
            && (!v.whole
                || mine
                    .iter()
                    .all(|pl| pl.params.keys().all(|k| v.params.iter().any(|(n, _)| n == k))))
    });
    if diff {
        h.apply_project();
    } else {
        h.reload();
    }
    Ok(())
}

fn alt_add(a: &str, h: &mut Host, copy: bool) -> Done {
    let verb = if copy { "copy" } else { "create" };
    let w = split_words(a);
    let Some(module) = w.first().map(|s| (*s).to_owned()) else {
        h.refuse([format!("usage: node alternative {verb} <node> [name]")]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match new_module_alternative(h.fs, &h.env, &dir, &module, w.get(1).copied(), copy) {
        Ok((_, made)) => {
            let what = if copy {
                "a copy of the one that was on"
            } else {
                "empty, and running"
            };
            h.log([format!("{module}: alternative '{made}' — {what}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node alternative {verb}: {e}")]),
    }
    Ok(())
}

/// `node alternative create <node> [name]` — an empty set, running.
pub fn alt_create(a: &str, h: &mut Host) -> Done {
    alt_add(a, h, false)
}

/// `node alternative copy <node> [name]` — a copy of the running set.
pub fn alt_copy(a: &str, h: &mut Host) -> Done {
    alt_add(a, h, true)
}

/// `node freeze <node>` — a toggle: a frozen node is thawed and its render deleted; freezing needs the host's render.
pub fn node_freeze(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let Some(module) = w.first().cloned() else {
        h.refuse(["usage: node freeze <node>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let node = resolve_node(&*h.fs, &h.env, &dir, &module)?;
    let current = node
        .as_ref()
        .and_then(|n| n.authored.frozen.as_ref())
        .map(|f| f.0.clone());
    if let Some(current) = current {
        set_module_frozen(h.fs, &h.env, &dir, &module, None)?;
        // Only a render inside the project is deleted: `frozen:` is text a file holds.
        let render = confine_to(&*h.fs, std::slice::from_ref(&dir), &join(&dir, &current));
        if let Some(r) = &render {
            h.fs.remove(r).map_err(|e| crate::host::Thrown(e.to_string()))?;
        }
        h.log([format!("{module}: unfrozen — everything upstream is live again")]);
        if render.is_none() {
            h.log([format!("  {current} is outside the project — left alone")]);
        }
        h.reload();
        return Ok(());
    }
    let lane = h.project()?.tracks.iter().any(|t| t.id == module);
    if !lane && chain_of_module(h, &module).is_none() {
        h.refuse([format!("freeze: no module \"{module}\" in the chains")]);
        return Ok(());
    }
    if !lane && !has_plugin(h, &module)? {
        h.refuse([format!(
            "freeze: '{module}' compiles to no plugin, so there is no signal at it to render"
        )]);
        return Ok(());
    }
    // The bounce must land before `frozen:` is written, and the crate cannot await the host's render.
    h.refuse([
        "freeze failed: no engine to render with — the bounce is the host's `render`, which a verb here cannot await"
            .to_owned(),
    ]);
    Ok(())
}
