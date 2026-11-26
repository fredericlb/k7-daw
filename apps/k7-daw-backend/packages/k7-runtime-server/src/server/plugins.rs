//! plugins.rs — the verbs about plugins: the installed catalogue and its scan, a plugin type's parameters, the live
//! instances of a project, their windows, presets and waveform taps.
//! What a reader gets wrong: a probe loads a binary, so it runs on a blocking task behind one of four slots and its
//! answer is a broadcast; a query read-only and identical to one still waiting joins it (`pending.rs`).

use super::ctx::{Ctx, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use super::plugin_scan::{When, detached, plugins_json, scan_in_background, status_json};
use super::supersede::superseded;
use crate::engine::Mt;
use crate::engine::state::ParamInfo;
use crate::log::say;
use serde_json::{Map, Value, json};

pub fn add(t: &mut HandlerTable) {
    t.add(&["plugin-scan"], scan_status);
    t.add(&["list-plugins", "rescan-plugins"], list_or_rescan);
    t.add(&["list-plugin-params"], list_plugin_params);
    t.add(&["list-edit-plugins"], list_edit_plugins);
    t.add(&["show-native-plugin-ui"], show_native_ui);
    t.add(&["show-native-plugin-ui-of"], show_native_ui_of);
    t.add(&["snapshot-plugin-base"], snapshot_base);
    t.add(&["set-plugin-param"], set_param);
    t.add(&["get-plugin-params"], get_params);
    t.add(&["list-presets"], list_presets);
    t.add(&["set-preset"], set_preset);
    t.add(&["subscribe-waveform", "unsubscribe-waveform"], waveform);
}

fn error(r: &Request, message: &str) {
    r.client.send(&json!({"type": "error", "message": message}));
}

const TOO_MANY: &str = "too many plugins loading at once — ask again in a moment";

/// Where the scan stands, to the asking socket alone; while one runs the same frame is broadcast, rate-limited.
fn scan_status(r: &Request) -> Result<(), Malformed> {
    r.client.send(&status_json(&r.ctx.plugins.board.snapshot()));
    Ok(())
}

/// `list-plugins` returns the cache, scanning once if cold; `rescan-plugins` always refreshes, detached because a
/// scan spawns a child per plugin and blocks. `filter` narrows by format, `force` re-scans all.
fn list_or_rescan(r: &Request) -> Result<(), Malformed> {
    let filter = r.msg.str("filter")?;
    let rescan = r.ty == "rescan-plugins";
    let force = r.msg.bool_or("force", false)?;
    let ctx = &r.ctx;
    let reply = {
        let (c, filter) = (ctx.clone(), filter.clone());
        move |found: &[crate::engine::plugins::PluginInfo]| {
            c.broadcast(&plugins_json(&c, found, &filter));
        }
    };
    // It is by far the slowest thing the engine does, so a cold cache turning a list into a minute of work must say why.
    if rescan {
        say!(
            "[plugins] rescan requested{} — scanning, this can take a while",
            if force { " (force)" } else { "" }
        );
        let when = if force { When::Force } else { When::Always };
        scan_in_background(ctx, when, reply);
        return Ok(());
    }
    let api = ctx.engine.api();
    let cached = api.load_plugin_cache();
    // Count the EXTERNALS, not the entries: built-ins are code, so a truncated cache still lists ~212 of them.
    let externals = cached.iter().filter(|p| p.format != "internal").count();
    if !api.plugin_cache_warm() {
        say!(
            "[plugins] exts.json cold ({} listed, {externals} external) — scanning once to fill it, this can take a while",
            cached.len()
        );
        scan_in_background(ctx, When::IfCold, reply);
    } else {
        let narrowed = if filter.is_empty() {
            String::new()
        } else {
            format!(" (filter '{filter}')")
        };
        say!(
            "[plugins] {} from cache ({externals} external){narrowed}",
            cached.len()
        );
        reply(&cached);
    }
    Ok(())
}

/// What a control needs to draw itself, `def` only when the plugin publishes a default.
fn range_json(p: &ParamInfo) -> Map<String, Value> {
    let mut one = Map::new();
    one.insert("min".into(), json!(p.min));
    one.insert("max".into(), json!(p.max));
    if let Some(def) = p.default {
        one.insert("def".into(), json!(def));
    }
    if !p.states.is_empty() {
        one.insert("states".into(), states_json(&p.states));
    }
    one
}

/// A parameter of a plugin TYPE: its name and its range.
fn spec_json(p: &ParamInfo) -> Value {
    let mut one = range_json(p);
    one.insert("name".into(), json!(p.name));
    Value::Object(one)
}

fn states_json(states: &[(String, f64)]) -> Value {
    Value::Array(
        states
            .iter()
            .map(|(label, value)| json!({"label": label, "value": value}))
            .collect(),
    )
}

/// Every parameter of an installed plugin TYPE, by instantiating it and asking: it runs detached like a scan and
/// broadcasts its reply, behind one of four slots.
fn list_plugin_params(r: &Request) -> Result<(), Malformed> {
    let (format, id) = (r.msg.str("format")?, r.msg.str("id")?);
    if format.is_empty() || id.is_empty() {
        error(r, "list-plugin-params needs a format and an id");
        return Ok(());
    }
    let Some(slot) = r.ctx.plugins.take_probe() else {
        error(r, TOO_MANY);
        return Ok(());
    };
    let ctx = r.ctx.clone();
    detached(&r.ctx, move || {
        // The slot goes back however the load ends, a panic included.
        let _slot = slot;
        let found = ctx.engine.api().plugin_params_of(&format, &id);
        let params: Vec<Value> = found.iter().flatten().map(spec_json).collect();
        let what = match &found {
            Some(_) => format!("{} parameters", params.len()),
            None => "could not be loaded".to_string(),
        };
        say!("[plugin-param-list] {id}: {what}");
        // `found` carries what an empty array cannot: could not load, versus loaded and publishes nothing.
        ctx.broadcast(
            &json!({"type": "plugin-param-list", "format": format, "id": id,
                              "found": found.is_some(), "params": params}),
        );
    });
    Ok(())
}

/// Every session's plugin instances for the supervisor's list, each row naming its session: engine-global.
fn list_edit_plugins(r: &Request) -> Result<(), Malformed> {
    let Some(ticket) = r.ctx.pending.admit("list-edit-plugins") else {
        return Ok(());
    };
    let ctx = r.ctx.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let mut rows = Vec::new();
        for sid in ctx.engine.session_ids() {
            for p in mt.plugin_instances(&sid) {
                rows.push(json!({"session": sid, "id": p.id, "name": p.name,
                                 "hasNativePluginUI": p.has_native_ui}));
            }
        }
        ctx.broadcast(&json!({"type": "edit-plugins", "plugins": rows}));
    });
    Ok(())
}

/// Opens a plugin's native editor window (external plugins only); the id is the PluginDoc id.
fn show_native_ui(r: &Request) -> Result<(), Malformed> {
    let id = r.msg.str("id")?;
    let session = r.session.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        say!("[native-plugin-ui] open '{id}'");
        mt.show_native_ui(&session, &id);
    });
    Ok(())
}

/// The same window for a plugin in NO project, on an orphan instance saved nowhere: the seconds-long load runs
/// detached, and only the window itself needs the message thread.
fn show_native_ui_of(r: &Request) -> Result<(), Malformed> {
    let (format, id) = (r.msg.str("format")?, r.msg.str("id")?);
    if format.is_empty() || id.is_empty() {
        error(r, "show-native-plugin-ui-of needs a format and an id");
        return Ok(());
    }
    let Some(slot) = r.ctx.plugins.take_probe() else {
        error(r, TOO_MANY);
        return Ok(());
    };
    let ctx = r.ctx.clone();
    detached(&r.ctx, move || {
        let _slot = slot;
        say!("[native-plugin-ui] loading '{id}'");
        // ANSWERED either way: a load can hang outright, and a caller told nothing cannot tell slow from dead.
        if !ctx.engine.api().prepare_native_ui(&format, &id) {
            say!("[native-plugin-ui] '{id}' did not load");
            opened(&ctx, &format, &id, false);
            return;
        }
        let c = ctx.clone();
        ctx.engine.post(move |mt: Mt<'_>| {
            let ok = mt.show_native_ui_of(&format, &id);
            say!(
                "[native-plugin-ui] {} '{id}'",
                if ok { "open" } else { "could not open" }
            );
            opened(&c, &format, &id, ok);
        });
    });
    Ok(())
}

fn opened(ctx: &Ctx, format: &str, id: &str, ok: bool) {
    ctx.broadcast(
        &json!({"type": "native-plugin-ui-opened", "format": format, "id": id, "ok": ok}),
    );
}

/// Keeps what the orphan window holds, as a .vstpreset at an absolute `path` the file service chose.
fn snapshot_base(r: &Request) -> Result<(), Malformed> {
    let (format, id, path) = (r.msg.str("format")?, r.msg.str("id")?, r.msg.str("path")?);
    if format.is_empty() || id.is_empty() || path.is_empty() {
        error(r, "snapshot-plugin-base needs a format, an id and a path");
        return Ok(());
    }
    let ctx = r.ctx.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        let ok = mt.snapshot_orphan_to(&format, &id, &path);
        say!(
            "[plugin-base] {} {path}",
            if ok { "wrote" } else { "could not write" }
        );
        // Answered rather than left for the caller to look: no instance open leaves no file at all.
        ctx.broadcast(
            &json!({"type": "plugin-base-saved", "format": format, "id": id,
                              "path": path, "ok": ok}),
        );
    });
    Ok(())
}

/// One parameter on the LIVE edit for a control being dragged; unlogged, arriving at interactive rates, and only
/// the newest value of a parameter still queued is worth setting.
fn set_param(r: &Request) -> Result<(), Malformed> {
    let (id, name) = (r.msg.str("id")?, r.msg.str("name")?);
    let value = r.msg.f64_or("value", 0.0)?;
    let session = r.session.clone();
    let (plugin, param) = (id.clone(), name.clone());
    r.ctx
        .supersede
        .post_param(&r.session, &plugin, &param, |ticket| {
            r.ctx.engine.post(move |mt: Mt<'_>| {
                if !superseded(&ticket) {
                    mt.set_plugin_param(&session, &id, &name, value);
                }
            });
        });
    Ok(())
}

/// Every automatable parameter of a live plugin with its CURRENT value, EVERY one, unlike the editor-close
/// broadcast's minimal preset; `meta` rides alongside as a separate object. The reply goes to the whole session.
fn get_params(r: &Request) -> Result<(), Malformed> {
    let id = r.msg.str("id")?;
    let key = format!("get-plugin-params\n{}\n{id}", r.session);
    let Some(ticket) = r.ctx.pending.admit(&key) else {
        return Ok(());
    };
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let mut params = Map::new();
        let mut meta = Map::new();
        for p in mt.plugin_param_infos(&session, &id) {
            params.insert(p.name.clone(), json!(p.value));
            meta.insert(p.name.clone(), Value::Object(range_json(&p)));
        }
        ctx.broadcast_to(
            &session,
            json!({"type": "plugin-params", "id": id, "params": params, "meta": meta}),
        );
    });
    Ok(())
}

/// The presets a module's external plugin offers, programs plus .vstpreset files, for the editor's dropdown.
fn list_presets(r: &Request) -> Result<(), Malformed> {
    let module = r.msg.str("module")?;
    let key = format!("list-presets\n{}\n{module}", r.session);
    let Some(ticket) = r.ctx.pending.admit(&key) else {
        return Ok(());
    };
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let rows: Vec<Value> = mt
            .list_presets(&session, &module)
            .into_iter()
            .map(|p| json!({"name": p.name, "source": p.source, "index": p.index, "path": p.path}))
            .collect();
        ctx.broadcast_to(
            &session,
            json!({"type": "presets", "module": module, "presets": rows}),
        );
    });
    Ok(())
}

/// Loads a preset into every live instance of `module` and persists it: `path` picks a file, else `index` a program.
fn set_preset(r: &Request) -> Result<(), Malformed> {
    let module = r.msg.str("module")?;
    let index = r.msg.i64_or("index", -1)? as i32;
    let path = r.msg.str("path")?;
    let session = r.session.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        let what = if path.is_empty() {
            format!("program {index}")
        } else {
            path.clone()
        };
        say!("[preset] {module} <- {what}");
        mt.set_preset(&session, &module, index, &path);
    });
    Ok(())
}

/// Subscribes a module's waveform tap by PluginDoc `id` and `side`, deliberately with no track index.
fn waveform(r: &Request) -> Result<(), Malformed> {
    let id = r.msg.str("id")?;
    let side = if r.msg.get("side")?.is_some() {
        r.msg.str("side")?
    } else {
        "output".to_string()
    };
    let on = r.ty == "subscribe-waveform";
    // `input` or `output`, nothing else: every other string would be one more tap and one more rebuild.
    if side != "input" && side != "output" {
        return Ok(());
    }
    if !id.is_empty() {
        let session = r.session.clone();
        r.ctx
            .engine
            .post(move |mt: Mt<'_>| mt.waveform_tap(&session, &id, &side, on));
    }
    Ok(())
}
