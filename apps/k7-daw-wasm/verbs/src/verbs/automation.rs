// automation.rs — a parameter bound to a curve: `node automate`, and `track automation add|remove`, its view from a lane; after nodeParam.ts, trackLane.ts.
// A reader would get wrong what `track automation remove` deletes: the binding only; the curve FILE stays, as it may drive others.

use super::curve::{bad_name, curve_rel, curves_dir};
use super::shared::{Done, join, meter, split_words};
use crate::host::Host;
use k7x::edit::js_number;
use song::doc::{PluginDoc, ProjectDoc};
use song::modules::{BuiltinParam, builtin_param_automatable, builtin_plugin};
use song_edit::project_edit::set_module_automation_binding;
use song_edit::start_time::to_position;
use song_edit::workspace_doc::ws_write_text;
use song_edit::workspace_file::ws_exists;

/// How an automation binding names a curve: `@meta/curves/sweep`.
fn curve_ref(id: &str) -> String {
    format!("@meta/curves/{id}")
}

/// The module half of a compiled plugin id, `<chain>:<module>`: the whole id when it has no colon.
fn plugin_module(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, m)| m)
}

/// A built-in plugin's parameters, or None for a plugin that is not one; the compiled `type` is the built-in's own spelling.
fn builtin_params(pl: &PluginDoc) -> Option<&'static [BuiltinParam]> {
    builtin_plugin(Some("internal"), Some(&pl.plugin_type)).map(|b| b.params.as_slice())
}

/// A curve id from a lane and a param: whatever a filename or a ref cannot carry becomes one `-`.
fn curve_id_for(track: &str, param: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in format!("{track}-{param}").chars() {
        let keep = c.is_ascii_alphanumeric() || c == '_' || c == '-';
        if keep {
            out.push(c);
        } else if !in_run {
            out.push('-');
        }
        in_run = !keep;
    }
    let out = out.strip_prefix('-').unwrap_or(&out);
    out.strip_suffix('-').unwrap_or(out).to_owned()
}

/// Every compiled plugin as its module and its type, the master run's included.
fn plugin_modules(p: &ProjectDoc) -> Vec<(&str, &str)> {
    p.tracks
        .iter()
        .flat_map(|t| t.plugins.iter())
        .chain(p.master_plugins.iter())
        .map(|pl| (plugin_module(&pl.id), pl.plugin_type.as_str()))
        .collect()
}

/// `node automate <module> <param> <curve> [trigger]` — points a parameter at a curve; `none` unbinds.
pub fn node_automate(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse([
            "usage: node automate <module> <param> <curve> [trigger]".to_owned(),
            "       (e.g. node automate osc Filter Freq sweep)".to_owned(),
        ]);
        return Ok(());
    }
    let module = w[0];
    let p = h.project()?;
    let known = plugin_modules(&p);
    // The last word is the curve unless it is ALSO a known module: that is a trigger.
    let has_trigger = w.len() >= 4 && known.iter().any(|(m, _)| *m == w[w.len() - 1]);
    let trigger = has_trigger.then(|| w[w.len() - 1]);
    let tail = if has_trigger { 2 } else { 1 };
    let curve = w[w.len() - tail];
    let param = w[1..w.len() - tail].join(" ");
    if param.is_empty() {
        h.refuse(["node automate: no parameter named between the module and the curve".to_owned()]);
        return Ok(());
    }
    // A built-in's plain-state control takes a value but no curve, so binding one is refused rather than ignored.
    if curve != "none"
        && let Some((_, kind)) = known.iter().find(|(m, _)| *m == module)
        && !builtin_param_automatable(kind, &param)
    {
        h.refuse([format!(
            "node automate: {kind}'s '{param}' cannot be automated — the plugin keeps it as plain state; pick an engine whose knobs automate"
        )]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let bound = (curve != "none").then(|| curve_ref(curve));
    match set_module_automation_binding(h.fs, &h.env, &dir, module, &param, bound.as_deref(), trigger) {
        Ok(path) => {
            h.reload();
            h.log([if curve == "none" {
                format!("{module}: {param} is no longer automated ({path})")
            } else {
                let fired = trigger.map_or_else(String::new, |t| format!(", fired by {t}"));
                format!("{module}: {param} <- {curve}{fired}")
            }]);
        }
        Err(e) => h.refuse([format!("node automate: {e}")]),
    }
    Ok(())
}

/// `track automation add|remove <track> <param>` — a view onto `node automate`; the param is everything after the track.
fn track_automation(a: &str, h: &mut Host, add: bool) -> Done {
    let verb = if add {
        "track automation add"
    } else {
        "track automation remove"
    };
    let w = split_words(a);
    if w.len() < 2 {
        h.refuse([
            format!("usage: {verb} <track> <param>"),
            format!("       (e.g. {verb} gtr Filter Freq — the param is everything after the track)"),
        ]);
        return Ok(());
    }
    let p = h.project()?;
    let (track_arg, param) = (w[0], w[1..].join(" "));
    let Some(track) = p.tracks.iter().find(|t| t.id == track_arg || t.name == track_arg) else {
        h.refuse([super::shared::no_track(&p, track_arg)]);
        return Ok(());
    };
    // Which module on this lane's chain declares that param; the compiled plugins carry the track they landed on.
    let owning: Vec<&PluginDoc> = track
        .plugins
        .iter()
        .filter(|pl| builtin_params(pl).is_some_and(|ps| ps.iter().any(|b| b.name == param)))
        .collect();
    if owning.is_empty() {
        let mut known: Vec<&str> = track
            .plugins
            .iter()
            .flat_map(|pl| builtin_params(pl).unwrap_or(&[]).iter().map(|b| b.name.as_str()))
            .collect();
        known.sort_unstable();
        known.dedup();
        h.refuse([
            format!("no parameter '{param}' on '{}'", track.id),
            if known.is_empty() {
                format!("  '{}' runs through no module with parameters", track.id)
            } else {
                format!("  it has: {}", known.join(", "))
            },
        ]);
        return Ok(());
    }
    if owning.len() > 1 {
        // Two knobs of one name on one chain is a real shape and there is no defensible way to pick.
        let names: Vec<&str> = owning.iter().map(|pl| plugin_module(&pl.id)).collect();
        h.refuse([
            format!(
                "'{param}' is on {} modules of '{}' ({}) — say which:",
                owning.len(),
                track.id,
                names.join(", ")
            ),
            format!("  node automate {} {param} <curve>", names[0]),
        ]);
        return Ok(());
    }
    let plugin = owning[0];
    let module = plugin_module(&plugin.id).to_owned();
    let dir = h.song_path.clone();
    if !add {
        match set_module_automation_binding(h.fs, &h.env, &dir, &module, &param, None, None) {
            Ok(path) => {
                h.reload();
                h.log([format!("{}: {param} is no longer automated ({path})", track.id)]);
            }
            Err(e) => h.refuse([format!("{verb}: {e}")]),
        }
        return Ok(());
    }
    let meta = builtin_params(plugin).and_then(|ps| ps.iter().find(|b| b.name == param));
    // Where the knob sits NOW, normalised: a flat curve at the wrong height is an audible jump, so refuse.
    let cur = plugin.params.get(&param).copied().or_else(|| meta.and_then(|m| m.def));
    let (Some(meta), Some(cur)) = (meta, cur) else {
        return refuse_unknown_height(h, &module, &param);
    };
    if meta.max <= meta.min {
        return refuse_unknown_height(h, &module, &param);
    }
    let value = (cur - meta.min) / (meta.max - meta.min);
    let id = curve_id_for(&track.id, &param);
    let (num, den) = meter(&p);
    // The song's end, so the flat line has both ends to grab; a song with no material yet gets one bar.
    let last = p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.start + c.length))
        .fold(num, f64::max);
    let track_id = track.id.clone();
    let done = flat_curve(h, &id, value, last, num, den).and_then(|rel| {
        set_module_automation_binding(h.fs, &h.env, &dir, &module, &param, Some(&curve_ref(&id)), None)
            .map(|path| (rel, path))
            .map_err(|e| e.to_string())
    });
    match done {
        Ok((rel, path)) => {
            h.reload();
            h.log([format!(
                "{track_id}: {param} <- {id} ({rel}), flat at {}% ({path})",
                js_number((value * 100.0).round())
            )]);
        }
        Err(e) => h.refuse([format!("{verb}: {e}")]),
    }
    Ok(())
}

fn refuse_unknown_height(h: &mut Host, module: &str, param: &str) -> Done {
    h.refuse([
        format!("can't tell where '{param}' currently sits, so a flat curve would move it"),
        format!("  set it once (node param {module} {param} <value>), or bind a curve yourself:"),
        format!("  curve new <name> && node automate {module} {param} <name>"),
    ]);
    Ok(())
}

/// Scaffolds `curves/<id>.k7x` as a FLAT custom curve at `value`, from the song's start to `end_beats`.
fn flat_curve(h: &mut Host, id: &str, value: f64, end_beats: f64, num: f64, den: f64) -> Result<String, String> {
    if let Some(why) = bad_name(id) {
        return Err(why);
    }
    let rel = curve_rel(id);
    let path = join(&h.song_path, &rel);
    if ws_exists(&*h.fs, &path) {
        return Err(format!("curve '{id}' already exists ({rel})"));
    }
    let v = js_number(((value.clamp(0.0, 1.0)) * 1e6).round() / 1e6);
    let end = to_position(end_beats, num, den).map_err(|e| e.to_string())?;
    let text = [
        "type(\"custom\")".to_owned(),
        "polarity(\"unipolar\") // unipolar 0..1 | bipolar -1..1".to_owned(),
        "points([".to_owned(),
        format!("  time(\"1:1:1\").value({v}),"),
        format!("  time({}).value({v}),", serde_json::Value::String(end)),
        "])".to_owned(),
        String::new(),
    ]
    .join("\n");
    curves_dir(h);
    ws_write_text(h.fs, &path, &text).map_err(|e| e.to_string())?;
    Ok(rel)
}

pub fn track_automation_add(a: &str, h: &mut Host) -> Done {
    track_automation(a, h, true)
}

pub fn track_automation_remove(a: &str, h: &mut Host) -> Done {
    track_automation(a, h, false)
}
