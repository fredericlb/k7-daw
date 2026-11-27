// edit.rs — what the grid editor does to a panel: tabs added, removed, renamed and moved; widgets cut, copied and pasted; a widget given another type.
// A reader would get wrong that a panel with ONE tab writes no `tabs:` at all: the second tab makes the list, and removing back to one folds it away.

use super::TAB_NAME_MAX;
use super::geometry::framed_by;
use super::loose::{Obj, jnum, kind, num, obj, span, with};
use crate::files::scalar::js_space;
use serde::Serialize;
use serde_json::{Map, Value, json};

/// The widgets of the page at `at` (every tab's shared ones when `None`): the map the grid writes.
pub fn widgets_at(ui: &Value, at: Option<usize>) -> Value {
    match at {
        None => ui.get("widgets").cloned().unwrap_or_else(|| json!({})),
        Some(i) => tab(ui, i)
            .and_then(|t| t.get("widgets").cloned())
            .unwrap_or_else(|| json!({})),
    }
}

fn tabs(ui: &Value) -> &[Value] {
    ui.get("tabs").and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn tab(ui: &Value, i: usize) -> Option<&Value> {
    tabs(ui).get(i)
}

/// Where the grid writes when nothing was chosen: the first tab of a tabbed panel, else the panel.
fn default_page(ui: &Value) -> Option<usize> {
    (!tabs(ui).is_empty()).then_some(0)
}

/// `at` checked against the document: nothing chosen, or a tab that has gone, sends the grid to the default.
/// `at` is `None` for undefined, `Some(None)` for the panel's own page and `Some(Some(i))` for a tab.
pub fn page_in(ui: &Value, at: Option<Option<usize>>) -> Option<usize> {
    match at {
        None => default_page(ui),
        Some(None) => None,
        Some(Some(i)) => {
            if tab(ui, i).is_some() {
                Some(i)
            } else {
                default_page(ui)
            }
        }
    }
}

/// `ui` with the map at `at` replaced; the other tabs are untouched.
pub fn write_widgets_at(ui: &Value, at: Option<usize>, next: Value) -> Value {
    let Some(i) = at else {
        return with(ui, "widgets", next);
    };
    if tab(ui, i).is_none() {
        return ui.clone();
    }
    let list: Vec<Value> = tabs(ui)
        .iter()
        .enumerate()
        .map(|(j, t)| {
            if j == i {
                with(t, "widgets", next.clone())
            } else {
                t.clone()
            }
        })
        .collect();
    with(ui, "tabs", Value::Array(list))
}

/// Every widget id on the panel, every tab's included, in the order they are met.
pub fn panel_ids(ui: &Value) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let maps = std::iter::once(obj(ui, "widgets")).chain(tabs(ui).iter().map(|t| obj(t, "widgets")));
    for m in maps {
        for id in m.into_iter().flat_map(|m| m.keys()) {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

/// A tab name as it may be written: trimmed and cut to `TAB_NAME_MAX` UTF-16 units.
fn tab_name(raw: &str) -> String {
    let mut units = 0;
    let mut out = String::new();
    for c in raw.trim_matches(js_space).chars() {
        units += c.len_utf16();
        if units > TAB_NAME_MAX {
            break;
        }
        out.push(c);
    }
    out
}

fn free_name(tabs: &[Value], base: &str) -> String {
    let taken: Vec<&str> = tabs
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();
    (tabs.len() + 1..)
        .map(|i| tab_name(&format!("{base} {i}")))
        .find(|n| !taken.contains(&n.as_str()))
        .unwrap_or_default()
}

/// The new tab's name when none was typed, and the first tab's when a second turns the panel's widgets into one.
const NEW_TAB: &str = "Tab";
const FIRST_TAB: &str = "Main";

/// A tab added at the end. The first one added makes two: the panel's widgets become tab one. Answers the tab to select.
pub fn add_tab(ui: &Value) -> (Value, usize) {
    let list = tabs(ui);
    if list.is_empty() {
        let first = json!({ "name": FIRST_TAB, "widgets": ui.get("widgets").cloned().unwrap_or_else(|| json!({})) });
        let name = free_name(std::slice::from_ref(&first), NEW_TAB);
        let next = with(ui, "widgets", json!({}));
        return (with(&next, "tabs", json!([first, { "name": name, "widgets": {} }])), 1);
    }
    let mut next = list.to_vec();
    next.push(json!({ "name": free_name(list, NEW_TAB), "widgets": {} }));
    let at = next.len() - 1;
    (with(ui, "tabs", Value::Array(next)), at)
}

/// Tab `at` removed with its widgets. Down to one, the list folds away: its widgets join the panel's.
pub fn remove_tab(ui: &Value, at: usize) -> Value {
    if tab(ui, at).is_none() {
        return ui.clone();
    }
    let rest: Vec<Value> = tabs(ui)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, t)| t.clone())
        .collect();
    if rest.len() > 1 {
        return with(ui, "tabs", Value::Array(rest));
    }
    let mut widgets = obj(ui, "widgets").cloned().unwrap_or_default();
    for (id, w) in rest.first().and_then(|t| obj(t, "widgets")).into_iter().flatten() {
        widgets.insert(id.clone(), w.clone());
    }
    let mut out = ui.as_object().cloned().unwrap_or_default();
    out.shift_remove("tabs");
    out.insert("widgets".into(), Value::Object(widgets));
    Value::Object(out)
}

/// Tab `at` renamed, or `ui` unchanged when the name is empty or another tab's.
pub fn rename_tab(ui: &Value, at: usize, raw: &str) -> Value {
    let name = tab_name(raw);
    if tab(ui, at).is_none() || name.is_empty() {
        return ui.clone();
    }
    let clash = tabs(ui)
        .iter()
        .enumerate()
        .any(|(i, t)| i != at && t.get("name").and_then(Value::as_str) == Some(&name));
    if clash {
        return ui.clone();
    }
    let list: Vec<Value> = tabs(ui)
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == at {
                with(t, "name", Value::String(name.clone()))
            } else {
                t.clone()
            }
        })
        .collect();
    with(ui, "tabs", Value::Array(list))
}

/// Tab `at` moved one place up (-1) or down (+1); the new position, or `at` when it cannot move.
pub fn move_tab(ui: &Value, at: usize, by: i64) -> (Value, usize) {
    let to = at as i64 + by;
    if tab(ui, at).is_none() || to < 0 || tab(ui, to as usize).is_none() {
        return (ui.clone(), at);
    }
    let mut list = tabs(ui).to_vec();
    list.swap(at, to as usize);
    (with(ui, "tabs", Value::Array(list)), to as usize)
}

/// What a copy holds: the widgets at clip-relative positions, and the block's size in cells.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Clip {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Value,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cols: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub rows: f64,
}

/// The ids a cut or a copy of `id` takes: a group brings what it frames, first in the list.
pub fn clip_ids(widgets: &Obj, id: &str) -> Vec<String> {
    let Some(w) = widgets.get(id) else {
        return Vec::new();
    };
    if kind(w) != "group" {
        return vec![id.to_owned()];
    }
    let mut ids = vec![id.to_owned()];
    ids.extend(framed_by(widgets, w));
    ids
}

/// A copy of `id` (and, for a group, its members), moved so the block starts at cell 0,0.
pub fn copy_widgets(widgets: &Obj, id: &str) -> Option<Clip> {
    let ids = clip_ids(widgets, id);
    if ids.is_empty() {
        return None;
    }
    let col = ids
        .iter()
        .map(|i| num(&widgets[i], "col"))
        .fold(f64::INFINITY, f64::min);
    let row = ids
        .iter()
        .map(|i| num(&widgets[i], "row"))
        .fold(f64::INFINITY, f64::min);
    let mut out = Map::new();
    let (mut cols, mut rows) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for i in &ids {
        let w = &widgets[i];
        let moved = with(
            &with(w, "col", jnum(num(w, "col") - col)),
            "row",
            jnum(num(w, "row") - row),
        );
        cols = cols.max(num(&moved, "col") + span(&moved, "colSpan"));
        rows = rows.max(num(&moved, "row") + span(&moved, "rowSpan"));
        out.insert(i.clone(), moved);
    }
    Some(Clip {
        widgets: Value::Object(out),
        cols,
        rows,
    })
}

/// The id itself when free, else `<id>_2`, `<id>_3`…: a pasted widget keeps a name a person can read.
fn free_id(id: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == id) {
        return id.to_owned();
    }
    (2..)
        .map(|k| format!("{id}_{k}"))
        .find(|n| !taken.contains(n))
        .unwrap_or_default()
}

/// `widgets` with `clip` dropped at `(col, row)`, pulled back inside the grid; the second answer is the block's head, to select.
pub fn paste_widgets(
    widgets: &Obj,
    clip: &Clip,
    at: (f64, f64),
    grid: (f64, f64),
    taken: &[String],
) -> (Value, Option<String>) {
    let col = at.0.min(grid.0 - clip.cols).max(0.0);
    let row = at.1.min(grid.1 - clip.rows).max(0.0);
    let mut next = widgets.clone();
    let mut used: Vec<String> = taken.to_vec();
    used.extend(widgets.keys().cloned());
    let mut first = None;
    for (id, w) in clip.widgets.as_object().into_iter().flatten() {
        let nid = free_id(id, &used);
        used.push(nid.clone());
        next.insert(
            nid.clone(),
            with(
                &with(w, "col", jnum(num(w, "col") + col)),
                "row",
                jnum(num(w, "row") + row),
            ),
        );
        first.get_or_insert(nid);
    }
    (Value::Object(next), first)
}

/// A fresh group's tint: the OP palette's blue, faint enough that the controls on it stay legible.
pub const GROUP_COLOR: &str = "#3d7dff26";

/// The widget kinds that carry a `bind`, and so the only ones a dragged parameter can bounce off.
pub const BINDABLE_WIDGET_TYPES: [&str; 10] = [
    "knob",
    "slider",
    "fader",
    "number",
    "toggle",
    "button",
    "meter",
    "dropdown",
    "filepicker",
    "tap",
];

/// The kinds that carry a range. Not a dropdown: it addresses its param by named setting, and its options are the set.
pub const RANGED_WIDGET_TYPES: [&str; 4] = ["knob", "slider", "fader", "number"];

/// Whether a kind carries a `bind`.
pub fn bindable(kind: &str) -> bool {
    !matches!(kind, "label" | "waveform" | "lcdscreen" | "group")
}

/// Whether a kind carries a range.
pub fn ranged(kind: &str) -> bool {
    RANGED_WIDGET_TYPES.contains(&kind)
}

/// A widget changed to `kind`, as the fields to set and the ones to clear: a leftover field fails validation and takes the graph down.
pub fn retype(w: &Value, kind: &str) -> (Map<String, Value>, Vec<&'static str>) {
    let keep = |key: &str, yes: bool| yes.then(|| w.get(key).filter(|v| !v.is_null()).cloned()).flatten();
    let r = ranged(kind);
    let color = (kind == "group").then(|| {
        w.get("color")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| json!(GROUP_COLOR))
    });
    let fields: [(&'static str, Option<Value>); 11] = [
        ("text", keep("text", kind == "label")),
        ("bind", keep("bind", bindable(kind))),
        ("min", keep("min", r)),
        ("max", keep("max", r)),
        ("step", keep("step", r)),
        ("curve", keep("curve", r)),
        ("source", keep("source", kind == "waveform")),
        ("options", keep("options", kind == "dropdown")),
        ("library", keep("library", kind == "dropdown")),
        ("color", color),
        ("pair", keep("pair", kind == "knob")),
    ];
    let mut set = Map::new();
    let mut clear = Vec::new();
    for (key, value) in fields {
        match value {
            Some(v) => {
                set.insert(key.to_owned(), v);
            }
            None => clear.push(key),
        }
    }
    (set, clear)
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
