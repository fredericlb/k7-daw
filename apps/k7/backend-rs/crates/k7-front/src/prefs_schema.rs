//! prefs_schema.rs — the backend's preferences schema (`prefsSchema` in `apps/k7/backend/src/server.ts`) as
//! zod applies it: known keys kept in the schema's order, unknown nested keys stripped, a wrong type an issue.
//! A field added there must be added here, which `apps/k7/backend-rs/docs/routes.md` lists as the cost.

use serde_json::{Map, Value};

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn expected(what: &str, got: &Value) -> String {
    format!(
        "Invalid input: expected {what}, received {}",
        type_name(got)
    )
}

/// One optional field: absent is fine, present must pass `check`, which returns the kept value.
fn field(
    from: &Map<String, Value>,
    key: &str,
    out: &mut Map<String, Value>,
    issues: &mut Vec<String>,
    check: impl Fn(&Value) -> Result<Value, String>,
) {
    if let Some(v) = from.get(key) {
        match check(v) {
            Ok(kept) => {
                out.insert(key.to_string(), kept);
            }
            Err(e) => issues.push(e),
        }
    }
}

fn string(v: &Value) -> Result<Value, String> {
    v.is_string()
        .then(|| v.clone())
        .ok_or_else(|| expected("string", v))
}

fn strings(v: &Value) -> Result<Value, String> {
    match v {
        Value::Array(items) if items.iter().all(Value::is_string) => Ok(v.clone()),
        Value::Array(items) => Err(items
            .iter()
            .find(|i| !i.is_string())
            .map(|i| expected("string", i))
            .unwrap_or_default()),
        _ => Err(expected("array", v)),
    }
}

fn command(v: &Value) -> Result<Value, String> {
    let Value::Object(o) = v else {
        return Err(expected("object", v));
    };
    let mut out = Map::new();
    let mut issues = Vec::new();
    field(o, "label", &mut out, &mut issues, |l| match l {
        Value::String(_) => Ok(l.clone()),
        Value::Object(m) if m.values().all(Value::is_string) => Ok(l.clone()),
        _ => Err("Invalid input".into()),
    });
    field(o, "icon", &mut out, &mut issues, string);
    field(o, "order", &mut out, &mut issues, |n| {
        n.is_number()
            .then(|| n.clone())
            .ok_or_else(|| expected("number", n))
    });
    field(o, "key", &mut out, &mut issues, string);
    field(o, "menu", &mut out, &mut issues, |m| match m {
        Value::Bool(_) => Ok(m.clone()),
        Value::Array(items) if items.iter().all(Value::is_string) => Ok(m.clone()),
        _ => Err("Invalid input".into()),
    });
    if issues.is_empty() {
        Ok(Value::Object(out))
    } else {
        Err(issues.join("; "))
    }
}

/// The document zod would keep, or its issues joined with "; ", as the route words a 400.
pub fn parse(sent: &Value) -> Result<Map<String, Value>, String> {
    let Value::Object(o) = sent else {
        return Err(expected("object", sent));
    };
    let mut out = Map::new();
    let mut issues = Vec::new();
    field(o, "engineId", &mut out, &mut issues, string);
    field(o, "commands", &mut out, &mut issues, |c| {
        let Value::Object(m) = c else {
            return Err(expected("record", c));
        };
        let mut kept = Map::new();
        for (k, v) in m {
            kept.insert(k.clone(), command(v)?);
        }
        Ok(Value::Object(kept))
    });
    field(o, "playbackTarget", &mut out, &mut issues, string);
    field(o, "favorites", &mut out, &mut issues, strings);
    if issues.is_empty() {
        Ok(out)
    } else {
        Err(issues.join("; "))
    }
}

/// Numbers as JavaScript prints them: an integral float loses its `.0`.
pub fn js_numbers(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15 => {
                Value::from(f as i64)
            }
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(js_numbers).collect()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, x)| (k.clone(), js_numbers(x))).collect())
        }
        _ => v.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_the_schema_order_and_strips_nested_strangers() {
        let sent = json!({ "favorites": ["a"], "commands": { "x": { "menu": false, "icon": "i", "extra": 1 } }, "engineId": "e" });
        let kept = parse(&sent).unwrap();
        assert_eq!(
            Value::Object(kept).to_string(),
            r#"{"engineId":"e","commands":{"x":{"icon":"i","menu":false}},"favorites":["a"]}"#
        );
    }

    #[test]
    fn a_wrong_type_is_an_issue_and_an_unknown_key_is_dropped() {
        assert_eq!(
            parse(&json!({ "engineId": 5 })).unwrap_err(),
            "Invalid input: expected string, received number"
        );
        assert_eq!(
            parse(&json!([])).unwrap_err(),
            "Invalid input: expected object, received array"
        );
        assert!(parse(&json!({ "engineId": null })).is_err());
        assert!(
            !parse(&json!({ "theme": "dark" }))
                .unwrap()
                .contains_key("theme")
        );
    }

    #[test]
    fn integral_floats_print_as_javascript_does() {
        assert_eq!(
            js_numbers(&json!({ "order": 1.0, "x": 1.5 })).to_string(),
            r#"{"order":1,"x":1.5}"#
        );
    }
}
