// write.rs — a panel read from and written to a module's k7x text: the model the grid edits, printed with the fewest edits, a widget a line.
// A reader would get wrong that a structure is ever rebuilt from scratch: only one that cannot be edited in place is (apps/k7-daw-frontend/packages/ui-module-builder/docs/builder.md).

use super::loose::{Obj, jnum, kind, obj};
use crate::files::validate_file;
use k7x::PathSeg;
use k7x::edit::{Document, StringifyOptions, Value as K7, WriteOptions, stringify};
use serde_json::{Map, Value};

/// A JSON value as k7x data, its maps ordered as the object it came from.
pub fn to_k7(j: &Value) -> K7 {
    match j {
        Value::Null => K7::Null,
        Value::Bool(b) => K7::Bool(*b),
        Value::Number(n) => K7::Num(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => K7::Str(s.clone()),
        Value::Array(a) => K7::List(a.iter().map(to_k7).collect()),
        Value::Object(m) => K7::object(m.iter().map(|(k, v)| (k.clone(), to_k7(v))).collect()),
    }
}

/// k7x data as JSON: a number that is whole prints as an integer, NaN as null.
pub fn from_k7(v: &K7) -> Value {
    match v {
        K7::Null => Value::Null,
        K7::Bool(b) => Value::Bool(*b),
        K7::Num(n) => jnum(*n),
        K7::Str(s) => Value::String(s.clone()),
        K7::List(a) => Value::Array(a.iter().map(from_k7).collect()),
        K7::Map(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), from_k7(v))).collect()),
    }
}

fn is_widget_at(path: &[PathSeg], value: &K7) -> bool {
    let n = path.len();
    n >= 2 && matches!(&path[n - 2], PathSeg::Key(k) if k == "widgets") && matches!(value.get("type"), Some(K7::Str(_)))
}

/// A whole `ui.k7x` for a panel's plain data, headed by `//` lines, each widget on one line.
pub fn panel_file_text(panel: &Value, header: &[String]) -> Result<String, String> {
    let inline = |p: &[PathSeg], v: &K7| is_widget_at(p, v);
    let opts = StringifyOptions {
        header,
        inline: Some(&inline),
    };
    stringify(&to_k7(panel), &opts).map_err(|e| e.to_string())
}

/// The source of a value as a statement's argument prints it, its widgets one a line.
pub fn panel_source(value: &Value) -> Result<String, String> {
    let wrapped = Value::Object(Map::from_iter([("v".to_owned(), value.clone())]));
    let text = panel_file_text(&wrapped, &[])?;
    let t = text.trim_end();
    Ok(t.get("v(".len()..t.len().saturating_sub(1)).unwrap_or("").to_owned())
}

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn at(path: &[PathSeg], rest: impl IntoIterator<Item = PathSeg>) -> Vec<PathSeg> {
    let mut p = path.to_vec();
    p.extend(rest);
    p
}

type Res<T> = Result<T, String>;

fn e(err: k7x::edit::EditError) -> String {
    err.to_string()
}

fn same(a: &K7, b: &K7) -> bool {
    match (a, b) {
        (K7::List(x), K7::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (K7::Map(x), K7::Map(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, w)| k == k2 && same(v, w)))
        }
        (K7::Null, K7::Null) => true,
        (K7::Bool(p), K7::Bool(q)) => p == q,
        (K7::Num(p), K7::Num(q)) => p == q,
        (K7::Str(p), K7::Str(q)) => p == q,
        _ => false,
    }
}

/// Edits a widget map in place; false when it cannot be, an empty or one-line map growing on its own line.
fn merge_widgets(doc: &mut Document, path: &[PathSeg], after: &Value) -> Res<bool> {
    let before = doc.get_in(path).map_err(e)?;
    let after_k = to_k7(after);
    if before.as_ref().is_some_and(|b| same(b, &after_k)) {
        return Ok(true);
    }
    let (Some(K7::Map(before)), Value::Object(after_m)) = (before, after) else {
        return Ok(false);
    };
    if before.is_empty() {
        return Ok(false);
    }
    if !doc.get_source(path).map_err(e)?.is_some_and(|s| s.contains('\n')) {
        return Ok(false);
    }
    for (id, _) in &before {
        if !after_m.contains_key(id) {
            doc.delete_in(&at(path, [key(id)]), false).map_err(e)?;
        }
    }
    for (id, w) in after_m {
        let was_map = before.iter().any(|(k, v)| k == id && matches!(v, K7::Map(_)));
        let target = at(path, [key(id)]);
        if was_map {
            doc.apply_in(&target, &to_k7(w)).map_err(e)?;
        } else {
            doc.set_in(&target, &to_k7(w), WriteOptions { inline: true })
                .map_err(e)?;
        }
    }
    Ok(true)
}

/// Edits a tab list in place when it keeps its length; false otherwise.
fn merge_tabs(doc: &mut Document, path: &[PathSeg], after: &Value) -> Res<bool> {
    let before = doc.get_in(path).map_err(e)?;
    if before.as_ref().is_some_and(|b| same(b, &to_k7(after))) {
        return Ok(true);
    }
    let (Some(K7::List(before)), Value::Array(after)) = (before, after) else {
        return Ok(false);
    };
    if before.len() != after.len() {
        return Ok(false);
    }
    for (i, tab) in after.iter().enumerate() {
        if !matches!(before[i], K7::Map(_)) {
            return Ok(false);
        }
        let name = tab.get("name").map_or(K7::Null, to_k7);
        doc.set_in(
            &at(path, [PathSeg::Index(i), key("name")]),
            &name,
            WriteOptions::default(),
        )
        .map_err(e)?;
        let widgets = tab.get("widgets").cloned().unwrap_or(Value::Null);
        if !merge_widgets(doc, &at(path, [PathSeg::Index(i), key("widgets")]), &widgets)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Edits a layout (`cols`, `rows`, `widgets`, `tabs`, `vertical`) in place; false when a part of it cannot be.
fn merge_layout(doc: &mut Document, path: &[PathSeg], after: &Value) -> Res<bool> {
    if !matches!(doc.get_in(path).map_err(e)?, Some(K7::Map(_))) {
        return Ok(false);
    }
    for k in ["cols", "rows"] {
        let v = after.get(k).map_or(K7::Null, to_k7);
        doc.set_in(&at(path, [key(k)]), &v, WriteOptions::default())
            .map_err(e)?;
    }
    let widgets = after.get("widgets").cloned().unwrap_or(Value::Null);
    if !merge_widgets(doc, &at(path, [key("widgets")]), &widgets)? {
        return Ok(false);
    }
    for k in ["tabs", "vertical"] {
        let target = at(path, [key(k)]);
        match after.get(k) {
            None => {
                doc.delete_in(&target, false).map_err(e)?;
            }
            Some(v) if k == "tabs" => {
                if !merge_tabs(doc, &target, v)? {
                    return Ok(false);
                }
            }
            Some(v) => {
                if !merge_layout(doc, &target, v)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// A widget as MINIMAL source: defaults and empty optionals dropped, so the block stays as terse as a hand-written one.
fn widget_to_plain(w: &Value) -> Value {
    let t = kind(w);
    let is_ranged = matches!(t, "knob" | "slider" | "fader" | "number");
    let truthy = |k: &str| -> Option<Value> {
        w.get(k)
            .filter(|v| {
                !matches!(v, Value::Null | Value::Bool(false)) && v.as_str() != Some("") && v.as_f64() != Some(0.0)
            })
            .cloned()
    };
    let mut out = Map::new();
    let mut put = |k: &str, v: Option<Value>| {
        if let Some(v) = v {
            out.insert(k.to_owned(), v);
        }
    };
    put("type", w.get("type").cloned());
    put("col", w.get("col").cloned());
    put("row", w.get("row").cloned());
    let big = |k: &str| w.get(k).filter(|v| v.as_f64().is_some_and(|n| n > 1.0)).cloned();
    put("colSpan", big("colSpan"));
    put("rowSpan", big("rowSpan"));
    put("label", truthy("label"));
    put("text", (t == "label").then(|| truthy("text")).flatten());
    put(
        "bind",
        (!matches!(t, "label" | "waveform" | "lcdscreen"))
            .then(|| truthy("bind"))
            .flatten(),
    );
    // Range fields belong to the numeric kinds only: a `min` on a toggle fails to load.
    for k in ["min", "max", "step", "curve"] {
        put(k, if is_ranged { w.get(k).cloned() } else { None });
    }
    put("hidden", truthy("hidden").map(|_| Value::Bool(true)));
    put(
        "source",
        (t == "waveform").then(|| {
            w.get("source")
                .cloned()
                .unwrap_or_else(|| Value::String("output".into()))
        }),
    );
    let options = w
        .get("options")
        .filter(|o| o.as_array().is_some_and(|a| !a.is_empty()))
        .cloned();
    put("options", (t == "dropdown").then_some(options).flatten());
    put("library", (t == "dropdown").then(|| truthy("library")).flatten());
    put("color", (t == "group").then(|| truthy("color")).flatten());
    let invert = (t == "group" && truthy("bind").is_some() && truthy("invert").is_some()).then_some(Value::Bool(true));
    put("invert", invert);
    put("pair", (t == "knob").then(|| truthy("pair")).flatten());
    Value::Object(out)
}

fn plain_widgets(ws: Option<&Obj>) -> Value {
    Value::Object(
        ws.into_iter()
            .flatten()
            .map(|(id, w)| (id.clone(), widget_to_plain(w)))
            .collect(),
    )
}

fn plain_tabs(tabs: Option<&Vec<Value>>) -> Option<Value> {
    let tabs = tabs.filter(|t| !t.is_empty())?;
    Some(Value::Array(
        tabs.iter()
            .map(|t| {
                let mut m = Map::new();
                m.insert("name".into(), t.get("name").cloned().unwrap_or(Value::Null));
                m.insert("widgets".into(), plain_widgets(obj(t, "widgets")));
                Value::Object(m)
            })
            .collect(),
    ))
}

fn plain_layout(l: &Value, vertical: bool) -> Value {
    let mut m = Map::new();
    for k in ["cols", "rows"] {
        if let Some(v) = l.get(k) {
            m.insert(k.to_owned(), v.clone());
        }
    }
    m.insert("widgets".into(), plain_widgets(obj(l, "widgets")));
    if let Some(t) = plain_tabs(l.get("tabs").and_then(Value::as_array)) {
        m.insert("tabs".into(), t);
    }
    if !vertical && let Some(v) = l.get("vertical").filter(|v| v.is_object()) {
        m.insert("vertical".into(), plain_layout(v, true));
    }
    Value::Object(m)
}

/// A panel as the plain data its file holds, minimal and pruned.
pub fn ui_to_plain(ui: &Value) -> Value {
    plain_layout(ui, false)
}

/// Rewrites the module's `ui(…)` block from the model, in place where it can be, every other line untouched.
fn write_ui_in(doc: &mut Document, ui: &Value) -> Res<()> {
    let plain = ui_to_plain(ui);
    let path = [key("ui")];
    if !merge_layout(doc, &path, &plain)? {
        doc.set_source_in(&path, &panel_source(&plain)?).map_err(e)?;
    }
    Ok(())
}

/// Rewrites a `ui.k7x`'s grid statement by statement, so its header comment survives.
fn write_file_in(doc: &mut Document, ui: &Value) -> Res<()> {
    let plain = ui_to_plain(ui);
    let put = |doc: &mut Document, k: &str| {
        let v = plain.get(k).map_or(K7::Null, to_k7);
        doc.set_in(&[key(k)], &v, WriteOptions::default()).map_err(e)
    };
    put(doc, "cols")?;
    put(doc, "rows")?;
    for k in ["widgets", "tabs", "vertical"] {
        let path = [key(k)];
        let Some(value) = plain.get(k) else {
            doc.delete_in(&path, false).map_err(e)?;
            continue;
        };
        let merged = match k {
            "widgets" => merge_widgets(doc, &path, value)?,
            "tabs" => merge_tabs(doc, &path, value)?,
            _ => merge_layout(doc, &path, value)?,
        };
        if !merged {
            doc.set_source_in(&path, &panel_source(value)?).map_err(e)?;
        }
    }
    Ok(())
}

/// The panel's text rewritten from the model; a buffer that does not parse is handed back unchanged. `file` is a whole `ui.k7x`.
pub fn write_ui(text: &str, ui: &Value, file: bool) -> Res<String> {
    let mut doc = Document::new(text);
    if !doc.errors().is_empty() {
        return Ok(text.to_owned());
    }
    if file {
        write_file_in(&mut doc, ui)?;
    } else {
        write_ui_in(&mut doc, ui)?;
    }
    Ok(doc.text().to_owned())
}

/// The panel the Rust reader accepts, cut down to the keys a panel has; None when it refuses it or the keys do not print.
fn checked_ui(raw: &K7) -> Option<Value> {
    let K7::Map(m) = raw else { return None };
    let mut ui = Vec::new();
    for k in ["cols", "rows", "widgets", "tabs", "vertical"] {
        if let Some((_, v)) = m.iter().find(|(name, _)| name == k) {
            ui.push((k.to_owned(), v.clone()));
        }
    }
    let ui = K7::Map(ui);
    let printed = stringify(&ui, &StringifyOptions::default()).ok()?;
    validate_file("modules/audio/x/ui.k7x", &printed)
        .ok
        .then(|| from_k7(&ui))
}

/// What reading a buffer's panel finds.
pub struct PanelRead {
    /// The panel, when there is one the loader's reader accepts.
    pub ui: Option<Value>,
    /// A panel is stated and unreadable: the grid says so rather than misreading it.
    pub malformed: bool,
}

/// The `ui(…)` block of a module file (`file` false) or the panel a `ui.k7x` declares (`file` true).
pub fn read_ui(text: &str, file: bool) -> PanelRead {
    let doc = Document::new(text);
    if !doc.errors().is_empty() {
        return PanelRead {
            ui: None,
            malformed: false,
        };
    }
    let raw = if file {
        doc.to_js().ok()
    } else {
        doc.get_in(&[key("ui")]).ok().flatten()
    };
    let Some(raw) = raw else {
        return PanelRead {
            ui: None,
            malformed: false,
        };
    };
    let stated = if file {
        raw.get("cols").is_some() || raw.get("widgets").is_some()
    } else {
        !matches!(raw, K7::Null)
    };
    let ui = checked_ui(&raw);
    PanelRead {
        malformed: stated && ui.is_none(),
        ui,
    }
}

/// The `params(…)` block as name → value as TEXT, never numbers: `1.0` stays `1.0`.
pub fn read_params(text: &str) -> Map<String, Value> {
    let doc = Document::new(text);
    let mut out = Map::new();
    if !doc.errors().is_empty() {
        return out;
    }
    let Ok(Some(K7::Map(entries))) = doc.get_in(&[key("params")]) else {
        return out;
    };
    for (name, v) in entries {
        let raw = match v {
            K7::Str(s) => s,
            K7::List(_) | K7::Map(_) => continue,
            _ => doc
                .get_source(&[key("params"), key(&name)])
                .ok()
                .flatten()
                .unwrap_or_default(),
        };
        out.insert(name, Value::String(raw));
    }
    out
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
