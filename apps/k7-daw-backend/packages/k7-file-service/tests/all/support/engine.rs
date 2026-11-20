//! Shared by the engine tests: the vectors, a clock turned in steps, and the console lines a machine asks to be said.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;

/// `tests/vectors/engine.json`, read once.
pub fn vectors() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/engine.json"
        ))
        .unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

/// A turned clock: milliseconds since a scenario began, as the TypeScript's fake timers count them.
#[derive(Clone, Copy)]
pub struct Clock(pub Instant);

impl Clock {
    pub fn new() -> Clock {
        Clock(Instant::now())
    }

    pub fn at(&self, ms: u64) -> Instant {
        self.0 + Duration::from_millis(ms)
    }
}

/// A trace row as the vectors write it: `[at, what]`.
pub fn rows(v: &Value) -> Vec<(u64, String)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| (r[0].as_u64().unwrap(), r[1].as_str().unwrap().to_string()))
        .collect()
}

/// A mismatch that shows the first row that differs, not two walls of text.
pub fn same_rows(name: &str, got: &[(u64, String)], want: &[(u64, String)]) {
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: row {i}");
    }
    assert_eq!(got.len(), want.len(), "{name}: rows {got:?}\nwant {want:?}");
}

/// A value with every whole float spelt as an integer, as JavaScript spells a number.
pub fn js_numbers(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15 => Value::from(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(js_numbers).collect()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, x)| (k.clone(), js_numbers(x))).collect())
        }
        _ => v.clone(),
    }
}
