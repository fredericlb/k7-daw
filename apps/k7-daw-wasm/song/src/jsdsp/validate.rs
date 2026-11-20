// jsdsp/validate.rs — a graph the engine's DspGraph::build can read, checked here so one it cannot fails in the compile.
// A reader would get wrong that it is strict: it refuses only what build errors on; the rest build tolerates, the message is a fixed sentence.
// NODE_TYPES is the one place a new engine node is added; tests/jsdsp.rs compares it with DspGraph.cpp.

use super::MAX_GRAPH;
use serde_json::Value;
use std::collections::HashSet;

/// How deep a graph may nest, as `kMaxSourceDepth` of DspGraph.cpp.
const MAX_DEPTH: usize = 32;

/// The engine's node words, as `DspGraph::build` spells them (DspGraph.cpp, the `type ==` chain).
pub const NODE_TYPES: [&str; 32] = [
    // control and analysis, lines 159 to 170
    "envelope",
    "todb",
    "fromdb",
    "lerp",
    "linmap",
    "geolerp",
    "clamp",
    "select",
    "add",
    "sub",
    "mul",
    "glide",
    // audio, lines 171 to 173
    "gain",
    "mix",
    "biquad",
    // MIDI, lines 175 to 177
    "transpose",
    "velocity",
    "mchannel",
    "mhold",
    "mstrike",
    "mpattern",
    "maccent",
    "mchance",
    "mdecay",
    // sample phase, lines 190 to 191
    "ain",
    "aout",
    "adelay",
    "amul",
    "aadd",
    "atanh",
    "alfo",
    "aallpass",
];

/// The depth DspGraph::fromJson counts: brackets outside strings.
fn too_deep(text: &str) -> bool {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for c in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'[' || c == b'{' {
            depth += 1;
            if depth > MAX_DEPTH {
                return true;
            }
        } else if (c == b']' || c == b'}') && depth > 0 {
            depth -= 1;
        }
    }
    false
}

/// What `juce::var::toString` gives a param's name, which is what the engine compares.
fn name_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => (if *b { "1" } else { "0" }).to_owned(),
        Some(Value::Number(n)) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(f)) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e6 => format!("{f:.1}"),
            (None, Some(f)) => f.to_string(),
            (None, None) => String::new(),
        },
        Some(Value::Array(_)) => "[Array]".to_owned(),
        Some(Value::Object(_)) => "Object".to_owned(),
    }
}

fn param(i: usize, p: &Value, seen: &mut HashSet<String>) -> Result<(), String> {
    // The engine skips what is not an object.
    let Some(o) = p.as_object() else { return Ok(()) };
    let name = name_text(o.get("name"));
    if name.is_empty() {
        return Err(format!("param {i} has no name"));
    }
    if name == "sr" {
        return Err(format!("param {i} is called `sr`, which is the sample rate"));
    }
    // An object name prints its own address, so two never collide.
    let is_object = matches!(o.get("name"), Some(Value::Object(_)));
    if !is_object && !seen.insert(name) {
        return Err(format!("param {i} is declared twice"));
    }
    Ok(())
}

fn node(i: usize, n: &Value) -> Result<(), String> {
    // The engine skips what is not an object.
    let Some(o) = n.as_object() else { return Ok(()) };
    match o.get("type") {
        Some(Value::String(t)) if NODE_TYPES.contains(&t.as_str()) => Ok(()),
        _ => Err(format!("node {i} has a type the engine does not know")),
    }
}

/// Whether the engine can read this text, as the sentence that says why not (decision 351).
pub fn validate_graph(text: &str) -> Result<(), String> {
    if text.len() as u64 > MAX_GRAPH {
        return Err("the graph is over 1 MiB".to_owned());
    }
    if too_deep(text) {
        return Err(format!("the graph nests deeper than {MAX_DEPTH} levels"));
    }
    let root: Value = serde_json::from_str(text).map_err(|_| "the graph is not valid JSON".to_owned())?;
    let Some(root) = root.as_object() else {
        return Err("the graph is not an object".to_owned());
    };
    let mut names: HashSet<String> = HashSet::new();
    if let Some(Value::Array(params)) = root.get("params") {
        for (i, p) in params.iter().enumerate() {
            param(i, p, &mut names)?;
        }
    }
    let Some(Value::Array(nodes)) = root.get("nodes") else {
        return Err("the graph has no `nodes` list".to_owned());
    };
    for (i, n) in nodes.iter().enumerate() {
        node(i, n)?;
    }
    Ok(())
}
