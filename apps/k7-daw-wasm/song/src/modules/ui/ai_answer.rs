// ai_answer.rs — the JSON the model answers AI organize with, and its turn into the `ui.k7x` text that lands in a buffer.
// A reader would get wrong that the model writes k7x: it writes the panel as JSON data, and `write.rs` prints it.
// Widgets come as LISTS carrying their id, since a structured output allows no map with keys of the model's choosing.

use super::ai_check::AiResult;
use super::write::panel_file_text;
use crate::files::scalar::js_space;
use crate::modules::{WAVEFORM_SOURCES, WIDGET_TYPES};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// One line of AI organize's stream: progress while Claude writes, a ping to keep a proxy awake, then exactly one `done` or `error`.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiEvent {
    Status { text: String },
    Ping,
    Done { result: AiResult },
    Error { error: String },
}

fn widget_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "type": { "type": "string", "enum": WIDGET_TYPES },
            "col": { "type": "integer" },
            "row": { "type": "integer" },
            "colSpan": { "type": "integer" },
            "rowSpan": { "type": "integer" },
            "label": { "type": "string" },
            "text": { "type": "string" },
            "bind": { "type": "string" },
            "pair": { "type": "string" },
            "min": { "type": "number" },
            "max": { "type": "number" },
            "step": { "type": "number" },
            "curve": { "type": "string", "enum": ["lin", "log"] },
            "hidden": { "type": "boolean" },
            "source": { "type": "string", "enum": WAVEFORM_SOURCES },
            "options": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "label": { "type": "string" }, "value": { "type": "number" } },
                    "required": ["label", "value"],
                    "additionalProperties": false,
                },
            },
            "library": { "type": "string" },
            "color": { "type": "string" },
            "invert": { "type": "boolean" },
        },
        "required": ["id", "type", "col", "row"],
        "additionalProperties": false,
    })
}

/// The JSON the model is constrained to: snake_case at the top, as the prompt names the fields.
pub fn ai_result_schema() -> Value {
    let widgets = json!({ "type": "array", "items": widget_schema() });
    let tabs = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": { "name": { "type": "string" }, "widgets": widgets },
            "required": ["name", "widgets"],
            "additionalProperties": false,
        },
    });
    let layout =
        json!({ "cols": { "type": "integer" }, "rows": { "type": "integer" }, "widgets": widgets, "tabs": tabs });
    let mut panel = layout.as_object().cloned().unwrap_or_default();
    panel.insert(
        "vertical".into(),
        json!({
            "type": "object",
            "properties": layout,
            "required": ["cols", "rows", "widgets"],
            "additionalProperties": false,
        }),
    );
    json!({
        "type": "object",
        "properties": {
            "header": { "type": "string" },
            "panel": {
                "type": "object",
                "properties": panel,
                "required": ["cols", "rows", "widgets"],
                "additionalProperties": false,
            },
            "widgets_js": { "type": "string" },
            "notes": { "type": "string" },
        },
        "required": ["header", "panel", "widgets_js", "notes"],
        "additionalProperties": false,
    })
}

/// A widget list as the id-keyed map the file holds; an id met twice is reported and its later widget dropped.
fn widget_map(list: &Value, place: &str, issues: &mut Vec<String>) -> Value {
    let Value::Array(items) = list else {
        return list.clone();
    };
    let mut out = Map::new();
    for item in items {
        let (Some(o), Some(id)) = (item.as_object(), item.get("id").and_then(Value::as_str)) else {
            return list.clone();
        };
        if out.contains_key(id) {
            issues.push(format!("{place}: widget id `{id}` is used twice"));
        } else {
            let rest: Map<String, Value> = o
                .iter()
                .filter(|(k, _)| *k != "id")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            out.insert(id.to_owned(), Value::Object(rest));
        }
    }
    Value::Object(out)
}

/// A JavaScript template's `${x}` for a value that may be missing.
fn shown(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".into(),
        Some(other) => other.to_string(),
        None => "undefined".into(),
    }
}

/// A layout with its widget lists made maps, its tabs' and its vertical twin's too.
fn layout_data(raw: &Map<String, Value>, place: &str, issues: &mut Vec<String>) -> Map<String, Value> {
    let mut out = Map::new();
    for k in ["cols", "rows"] {
        if let Some(v) = raw.get(k) {
            out.insert(k.to_owned(), v.clone());
        }
    }
    if let Some(w) = raw.get("widgets") {
        out.insert("widgets".into(), widget_map(w, place, issues));
    }
    if let Some(Value::Array(tabs)) = raw.get("tabs").filter(|t| t.as_array().is_some_and(|a| !a.is_empty())) {
        let mapped = tabs
            .iter()
            .map(|t| match t.as_object() {
                Some(o) => {
                    let mut tab = Map::new();
                    if let Some(n) = o.get("name") {
                        tab.insert("name".into(), n.clone());
                    }
                    if let Some(w) = o.get("widgets") {
                        let at = format!("tab {}", shown(o.get("name")));
                        tab.insert("widgets".into(), widget_map(w, &at, issues));
                    }
                    Value::Object(tab)
                }
                None => t.clone(),
            })
            .collect();
        out.insert("tabs".into(), Value::Array(mapped));
    }
    if let Some(Value::Object(v)) = raw.get("vertical") {
        out.insert("vertical".into(), Value::Object(layout_data(v, "vertical", issues)));
    }
    out
}

/// A model sometimes fences a file it was told to return bare; the fence would not compile.
fn strip_fence(s: &str) -> String {
    let lead = s.trim_start_matches(js_space);
    let opened = lead.strip_prefix("```").and_then(|rest| {
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_lowercase());
        rest.strip_prefix('\n')
    });
    let s = opened.unwrap_or(s);
    let mut from = 0;
    while let Some(at) = s[from..].find("\n```") {
        let i = from + at;
        if s[i + 4..].chars().all(js_space) {
            return format!("{}\n", &s[..i]);
        }
        from = i + 1;
    }
    s.to_owned()
}

/// The model's raw answer, read off its JSON text and printed as `ui.k7x`; None when it is not the promised shape.
pub fn read_ai_answer(text: &str) -> Option<AiResult> {
    let raw: Value = serde_json::from_str(text).ok()?;
    let panel = raw.get("panel")?.as_object()?;
    let script = raw.get("widgets_js")?.as_str()?;
    let mut issues = Vec::new();
    let data = layout_data(panel, "panel", &mut issues);
    let header = raw
        .get("header")
        .and_then(Value::as_str)
        .map(|h| h.trim_matches(js_space))
        .unwrap_or("");
    let headers: Vec<String> = if header.is_empty() {
        Vec::new()
    } else {
        vec![header.to_owned()]
    };
    let ui_text = panel_file_text(&Value::Object(data), &headers).ok()?;
    Some(AiResult {
        ui_text,
        widgets_js: strip_fence(script),
        notes: raw.get("notes").and_then(Value::as_str).unwrap_or("").to_owned(),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fence_around_a_script_is_dropped() {
        assert_eq!(strip_fence("```js\nexport default {}\n```"), "export default {}\n");
        assert_eq!(strip_fence("export default {}"), "export default {}");
        assert_eq!(strip_fence("  ```\nx\n```  \n"), "x\n");
    }

    #[test]
    fn widgets_come_as_a_list_and_leave_as_a_map() {
        let answer = json!({
            "header": " m — a thing ",
            "panel": { "cols": 2, "rows": 2, "widgets": [
                { "id": "a", "type": "knob", "col": 0, "row": 0, "bind": "A" },
                { "id": "a", "type": "knob", "col": 1, "row": 0 },
            ] },
            "widgets_js": "",
            "notes": "n",
        });
        let got = read_ai_answer(&answer.to_string()).unwrap();
        assert!(got.ui_text.starts_with("// m — a thing\n"), "{}", got.ui_text);
        assert!(
            got.ui_text
                .contains("a: { type: \"knob\", col: 0, row: 0, bind: \"A\" }"),
            "{}",
            got.ui_text
        );
        assert_eq!(got.issues, vec!["panel: widget id `a` is used twice".to_owned()]);
        assert!(read_ai_answer("not json").is_none());
        assert!(read_ai_answer(r#"{"panel":{}}"#).is_none());
    }
}
