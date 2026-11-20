//! conflicts.rs — the alternatives a room set aside and nobody settled yet (`@studio/live-sync`'s `conflicts.ts`).
//! What a reader gets wrong: a newer alternative from the same replica for the same value replaces the older one.

use indexmap::IndexMap;
use serde_json::Value;

use crate::protocol::js_string;

/// How many open conflicts a workspace keeps; past that, the oldest go (they stay in the log).
const OPEN_KEEP: usize = 500;

/// `[a, b].join("\0")` of JSON values: null and undefined are empty, anything else `String(v)`.
fn join_nul<'a>(parts: impl Iterator<Item = Option<&'a Value>>) -> String {
    parts
        .map(|p| match p {
            None | Some(Value::Null) => String::new(),
            p => js_string(p),
        })
        .collect::<Vec<_>>()
        .join("\0")
}

/// The value a set-aside op is about: a file, or a path inside one.
fn value_key(op: Option<&Value>) -> String {
    let file = op.and_then(|o| o.get("file"));
    if op.and_then(|o| o.get("kind")).and_then(Value::as_str) != Some("k7x") {
        return join_nul(std::iter::once(file));
    }
    let path = op
        .and_then(|o| o.get("ops"))
        .and_then(|o| o.get(0))
        .and_then(|o| o.get("path"))
        .and_then(Value::as_array);
    join_nul(std::iter::once(file).chain(path.into_iter().flatten().map(Some)))
}

/// The open conflicts, by id, oldest first.
#[derive(Default, Debug, Clone)]
pub struct ConflictSet {
    open: IndexMap<String, Value>,
}

impl ConflictSet {
    pub fn list(&self) -> impl Iterator<Item = &Value> {
        self.open.values()
    }

    pub fn len(&self) -> usize {
        self.open.len()
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Replace every open conflict.
    pub fn reset(&mut self, list: &[Value]) {
        self.open.clear();
        for c in list {
            self.open.insert(js_string(c.get("id")), c.clone());
        }
    }

    /// Follow one ordered entry: what it settles closes, what it set aside opens.
    pub fn record(&mut self, entry: &Value) {
        let resolves = entry.pointer("/batch/resolves").and_then(Value::as_array);
        for id in resolves.into_iter().flatten().filter_map(Value::as_str) {
            self.open.shift_remove(id);
        }
        for c in entry
            .get("parked")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = value_key(c.get("op"));
            self.open.retain(|_, o| {
                !(o.get("replica") == c.get("replica") && value_key(o.get("op")) == key)
            });
            self.open.insert(js_string(c.get("id")), c.clone());
        }
        while self.open.len() > OPEN_KEEP {
            self.open.shift_remove_index(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parked(id: &str, replica: &str, op: Value) -> Value {
        json!({ "id": id, "seq": 1, "replica": replica, "ts": 0, "label": "", "op": op })
    }

    #[test]
    fn a_newer_alternative_replaces_the_older_one() {
        let mut set = ConflictSet::default();
        let k = |v: &str| json!({ "kind": "k7x", "file": "a.k7x", "ops": [{ "op": "set", "path": ["x"], "k7x": v }] });
        set.record(&json!({ "batch": {}, "parked": [parked("1.0", "r", k("1")), parked("1.1", "s", k("2"))] }));
        set.record(&json!({ "batch": {}, "parked": [parked("2.0", "r", k("3"))] }));
        let ids: Vec<_> = set
            .list()
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, ["1.1", "2.0"]);
        set.record(&json!({ "batch": { "resolves": ["1.1", 7] }, "parked": [] }));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn keeps_the_newest_five_hundred() {
        let mut set = ConflictSet::default();
        for i in 0..510 {
            let op = json!({ "kind": "text", "file": format!("f{i}") });
            set.record(&json!({ "batch": {}, "parked": [parked(&format!("{i}.0"), "r", op)] }));
        }
        assert_eq!(set.len(), 500);
        assert_eq!(set.list().next().unwrap()["id"], "10.0");
    }
}
