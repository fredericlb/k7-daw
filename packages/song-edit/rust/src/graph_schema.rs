// graph_schema.rs — graph.k7x's shape, as the writers read it: the expanded data in the schema's key order, or the first finding.
// A reader would get wrong the findings: Zod's wording is approximated, and only the first one is ever shown.

use crate::sugar::sugar_for;
use k7x::edit::{Document, Value};

pub const GRAPH_FILE: &str = "graph.k7x";
pub const GRAPH_BASE: &str = "graph";
pub(crate) const PORT_SEP: char = '#';

/// The schema's keys, in the order Zod writes them out.
const KEYS: [&str; 5] = ["modules", "bypassed", "bus_only", "tap_feeds", "positions"];

pub(crate) type Data = Vec<(String, Value)>;

/// A graph file as the schema reads it: its data, or the first finding.
pub struct ParsedGraph {
    pub value: Option<Value>,
    pub issues: Vec<String>,
}

fn received(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Num(_) => "number",
        Value::Str(_) => "string",
        Value::List(_) => "array",
        Value::Map(_) => "object",
    }
}

fn expected<T>(what: &str, v: &Value) -> Result<T, String> {
    Err(format!("Invalid input: expected {what}, received {}", received(v)))
}

fn name(v: &Value) -> Result<(), String> {
    match v {
        Value::Str(s) if s.is_empty() => Err("Too small: expected string to have >=1 characters".to_owned()),
        Value::Str(_) => Ok(()),
        other => expected("string", other),
    }
}

fn names(v: &Value) -> Result<(), String> {
    let Value::List(l) = v else { return expected("array", v) };
    l.iter().try_for_each(name)
}

fn dests(v: &Value) -> Result<(), String> {
    let Value::List(l) = v else { return expected("array", v) };
    for d in l {
        name(d)?;
        if let Value::Str(s) = d
            && (s.starts_with(PORT_SEP) || s.ends_with(PORT_SEP))
        {
            return Err("a port ref is `<module>#<port>` — neither half may be empty (see docs/ports.md)".to_owned());
        }
    }
    Ok(())
}

fn strict(v: &Value, allowed: &[&str]) -> Result<(), String> {
    let Value::Map(m) = v else { return expected("object", v) };
    let extra: Vec<String> = m
        .iter()
        .map(|(k, _)| k.as_str())
        .filter(|k| !allowed.contains(k))
        .map(|k| format!("\"{k}\""))
        .collect();
    match extra.len() {
        0 => Ok(()),
        1 => Err(format!("Unrecognized key: {}", extra[0])),
        _ => Err(format!("Unrecognized keys: {}", extra.join(", "))),
    }
}

fn record(v: &Value, each: fn(&Value) -> Result<(), String>) -> Result<(), String> {
    let Value::Map(m) = v else { return expected("object", v) };
    for (k, x) in m {
        if k.is_empty() {
            return Err("Too small: expected string to have >=1 characters".to_owned());
        }
        each(x)?;
    }
    Ok(())
}

fn feed(v: &Value) -> Result<(), String> {
    strict(v, &["node", "tap", "to"])?;
    ["node", "tap", "to"].iter().try_for_each(|k| {
        v.get(k).map_or_else(
            || Err("Invalid input: expected string, received undefined".to_owned()),
            name,
        )
    })
}

fn pin(v: &Value) -> Result<(), String> {
    strict(v, &["x", "y"])?;
    ["x", "y"].iter().try_for_each(|k| match v.get(k) {
        Some(Value::Num(_)) => Ok(()),
        Some(o) => expected("number", o),
        None => Err("Invalid input: expected number, received undefined".to_owned()),
    })
}

/// `GraphFileSchema.safeParse`: the data in the schema's key order, or the first finding.
fn validate(data: &Value) -> Result<Data, String> {
    strict(data, &KEYS)?;
    let mut out = Data::new();
    for k in KEYS {
        let Some(v) = data.get(k) else { continue };
        match k {
            "modules" => record(v, dests)?,
            "bypassed" | "bus_only" => names(v)?,
            "tap_feeds" => match v {
                Value::List(l) => l.iter().try_for_each(feed)?,
                o => return expected("array", o),
            },
            _ => record(v, pin)?,
        }
        out.push((k.to_owned(), v.clone()));
    }
    Ok(out)
}

/// The expanded data of a graph, validated; the short form's own message when it does not expand.
pub(crate) fn check(raw: &Value) -> Result<Data, String> {
    let expanded = match sugar_for(GRAPH_FILE) {
        Some(s) => s.try_expand(raw).map_err(|e| e.to_string())?,
        None => raw.clone(),
    };
    validate(&expanded)
}

/// A graph file's k7x text as its segments, runs expanded, with every finding.
#[must_use]
pub fn parse_graph(text: &str) -> ParsedGraph {
    let doc = Document::new(text);
    let read = doc.to_js().map_err(|e| e.to_string()).and_then(|raw| check(&raw));
    match read {
        Ok(d) => ParsedGraph {
            value: Some(Value::Map(d)),
            issues: vec![],
        },
        Err(m) => ParsedGraph {
            value: None,
            issues: vec![m],
        },
    }
}

/// Every node the file names: lanes and modules alike, out of one map.
#[must_use]
pub fn node_names(text: &str) -> Vec<String> {
    match parse_graph(text).value.as_ref().and_then(|v| v.get("modules")) {
        Some(Value::Map(m)) => m.iter().map(|(k, _)| k.clone()).collect(),
        _ => vec![],
    }
}
