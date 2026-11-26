//! msg.rs — a client's message as the C++ server read it: the text parsed with its nesting bounded, and
//! `value(key, default)` with nlohmann's refusals, so a handler's wrong-typed field is a `Malformed` answer.
//! What a reader gets wrong: the bound is counted on the text before the parse, since a copy or a walk of
//! thousands of levels overflows a connection's stack; 64 levels are allowed, the 65th is refused.

use serde_json::Value;

/// Deeper than this a message is refused (`kMaxMessageDepth`).
pub const MAX_DEPTH: usize = 64;

/// What `what()` of a thrown nlohmann exception said: the text after `malformed message: `.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

impl Malformed {
    fn type_error(code: u16, what: String) -> Self {
        Malformed(format!("[json.exception.type_error.{code}] {what}"))
    }
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `text` nests deeper than `MAX_DEPTH` outside its strings.
fn too_deep(text: &str) -> bool {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for b in text.bytes() {
        if in_string {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// The text of a message as JSON, or the reason the client is told after `bad json: `.
pub fn parse(text: &str) -> Result<Value, String> {
    if too_deep(text) {
        return Err(format!("message nested deeper than {MAX_DEPTH} levels"));
    }
    serde_json::from_str(text).map_err(|e| format!("[json.exception.parse_error.101] {e}"))
}

/// nlohmann's name for a value's type, as its errors print it.
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

/// A parsed message: reads a field the way `j.value(key, default)` did, refusing what nlohmann refused.
#[derive(Debug, Clone)]
pub struct Msg(pub Value);

impl Msg {
    /// The field, or `None` when absent; an error when the message is not an object (nlohmann 306).
    pub fn get(&self, key: &str) -> Result<Option<&Value>, Malformed> {
        match &self.0 {
            Value::Object(map) => Ok(map.get(key)),
            other => Err(Malformed::type_error(
                306,
                format!("cannot use value() with {}", type_name(other)),
            )),
        }
    }

    fn typed<'a, T>(
        &'a self,
        key: &str,
        want: &str,
        pick: impl FnOnce(&'a Value) -> Option<T>,
        default: T,
    ) -> Result<T, Malformed> {
        let Some(v) = self.get(key)? else {
            return Ok(default);
        };
        pick(v).ok_or_else(|| {
            Malformed::type_error(302, format!("type must be {want}, but is {}", type_name(v)))
        })
    }

    pub fn str(&self, key: &str) -> Result<String, Malformed> {
        self.typed(
            key,
            "string",
            |v| v.as_str().map(str::to_owned),
            String::new(),
        )
    }

    pub fn bool_or(&self, key: &str, default: bool) -> Result<bool, Malformed> {
        self.typed(key, "boolean", Value::as_bool, default)
    }

    pub fn f64_or(&self, key: &str, default: f64) -> Result<f64, Malformed> {
        self.typed(key, "number", Value::as_f64, default)
    }

    pub fn i64_or(&self, key: &str, default: i64) -> Result<i64, Malformed> {
        self.typed(
            key,
            "number",
            |v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)),
            default,
        )
    }
}
