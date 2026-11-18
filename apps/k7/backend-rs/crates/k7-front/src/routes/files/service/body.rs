//! body.rs — a request as the file service's Express reads it: the query as `String(req.query.x ?? "")`, the
//! body as `express.json` parses it (`{}` when there is none) and checked as each route's zod schema is, with
//! zod's own messages joined by `; ` (`validate` in `@studio/http/route`).
//! What a reader gets wrong: a field zod declares optional may be absent, never null.

use serde_json::{Map, Value};

/// The largest body `express.json({ limit: "8mb" })` takes.
pub const JSON_LIMIT: usize = 8 << 20;

/// `String(req.query[name] ?? "")`: a repeated key is its values joined by commas, as an array's `String`.
pub fn query(q: &str, name: &str) -> String {
    url::form_urlencoded::parse(q.as_bytes())
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .collect::<Vec<_>>()
        .join(",")
}

/// A JSON body as Express hands it on: `{}` without one or with another content type, else the value parsed.
pub fn json(content_type: Option<&str>, bytes: &[u8]) -> Result<Value, String> {
    let is_json = content_type
        .map(|t| {
            t.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        })
        .unwrap_or(false);
    if !is_json || bytes.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("invalid JSON: {e}"))?;
    // body-parser's `strict`: only an object or an array is a body.
    if !v.is_object() && !v.is_array() {
        return Err("invalid JSON: only an object or an array is a body".into());
    }
    Ok(v)
}

/// One field of a schema.
#[derive(Clone, Copy)]
pub enum Field {
    /// `z.string()`.
    Str,
    /// `z.string().min(1)`.
    Text,
    /// `z.string().optional()`.
    OptStr,
    /// `z.string().min(1).optional()`.
    OptText,
    /// `z.boolean()`.
    Bool,
    /// `z.boolean().optional()`.
    OptBool,
    /// `z.number()`.
    Num,
    /// `z.enum([...])`.
    Enum(&'static [&'static str]),
    /// `z.looseObject({})`.
    Object,
}

/// zod's name for what it received.
fn received(v: Option<&Value>) -> &'static str {
    match v {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn expected(what: &str, v: Option<&Value>) -> String {
    format!("Invalid input: expected {what}, received {}", received(v))
}

fn one_of(options: &[&str]) -> String {
    if let [only] = options {
        return format!("Invalid input: expected \"{only}\"");
    }
    let list: Vec<String> = options.iter().map(|o| format!("\"{o}\"")).collect();
    format!("Invalid option: expected one of {}", list.join("|"))
}

fn issues(field: Field, v: Option<&Value>, out: &mut Vec<String>) {
    let optional = matches!(field, Field::OptStr | Field::OptText | Field::OptBool);
    if v.is_none() && optional {
        return;
    }
    match field {
        Field::Str | Field::OptStr => {
            if !v.is_some_and(Value::is_string) {
                out.push(expected("string", v));
            }
        }
        Field::Text | Field::OptText => match v.and_then(Value::as_str) {
            Some("") => out.push("Too small: expected string to have >=1 characters".into()),
            Some(_) => {}
            None => out.push(expected("string", v)),
        },
        Field::Object => {
            if !matches!(v, Some(Value::Object(_))) {
                out.push(expected("object", v));
            }
        }
        Field::Bool | Field::OptBool => {
            if !v.is_some_and(Value::is_boolean) {
                out.push(expected("boolean", v));
            }
        }
        Field::Num => {
            if !v.is_some_and(Value::is_number) {
                out.push(expected("number", v));
            }
        }
        Field::Enum(options) => {
            if !v
                .and_then(Value::as_str)
                .is_some_and(|s| options.contains(&s))
            {
                out.push(one_of(options));
            }
        }
    }
}

/// The body checked against `schema`, as `validate` answers it: every issue's message joined by `; `.
pub fn check(body: &Value, schema: &[(&str, Field)]) -> Result<(), String> {
    let Some(map) = body.as_object() else {
        return Err(expected("object", Some(body)));
    };
    let mut out = Vec::new();
    for (name, field) in schema {
        issues(*field, map.get(*name), &mut out);
    }
    if out.is_empty() {
        Ok(())
    } else {
        Err(out.join("; "))
    }
}

/// A string field the schema already checked.
pub fn text(body: &Value, name: &str) -> String {
    body.get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const WRITE: [(&str, Field); 3] = [
        ("path", Field::Text),
        ("content", Field::Str),
        ("base", Field::OptStr),
    ];

    #[test]
    fn answers_zods_own_messages() {
        let both = "Invalid input: expected string, received undefined";
        assert_eq!(check(&json!({}), &WRITE), Err(format!("{both}; {both}")));
        assert_eq!(
            check(&json!({ "path": "" }), &WRITE),
            Err(format!(
                "Too small: expected string to have >=1 characters; {both}"
            ))
        );
        assert_eq!(
            check(&json!({ "path": "a", "content": "b", "base": 3 }), &WRITE),
            Err("Invalid input: expected string, received number".into())
        );
        assert_eq!(
            check(&json!([]), &WRITE),
            Err("Invalid input: expected object, received array".into())
        );
        let shape = [(
            "shape",
            Field::Enum(&["arrangement", "mix", "video", "live"]),
        )];
        assert_eq!(
            check(&json!({ "shape": "x" }), &shape),
            Err(
                "Invalid option: expected one of \"arrangement\"|\"mix\"|\"video\"|\"live\"".into()
            )
        );
        assert_eq!(
            check(&json!({ "confirm": 1 }), &[("confirm", Field::OptBool)]),
            Err("Invalid input: expected boolean, received number".into())
        );
        assert_eq!(
            check(&json!({ "path": "a", "content": "" }), &WRITE),
            Ok(())
        );
    }

    #[test]
    fn reads_the_query_and_body_as_express_does() {
        assert_eq!(query("path=a%20b&x=1", "path"), "a b");
        assert_eq!(query("path=a&path=b", "path"), "a,b");
        assert_eq!(query("", "path"), "");
        assert_eq!(json(None, b"{\"a\":1}"), Ok(json!({})));
        assert_eq!(
            json(Some("application/json; charset=utf-8"), b"{\"a\":1}"),
            Ok(json!({ "a": 1 }))
        );
        assert_eq!(json(Some("application/json"), b""), Ok(json!({})));
        assert!(json(Some("application/json"), b"3").is_err());
    }
}
