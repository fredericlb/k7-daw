//! room.rs — one workspace's ordered history (`@studio/live-sync`'s `room.ts`): the order every entry takes, and the
//! canonical text. An op is applied HERE before it is logged; one whose value moved meanwhile is SET ASIDE as a
//! conflict. Entries stay the JSON objects they arrived as, key order kept.
//! What a reader gets wrong: a batch's own fields, even unknown ones, travel in its entry exactly as they came.

use std::collections::VecDeque;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use crate::bases::{TextBases, Version};
use crate::conflicts::ConflictSet;
use crate::json::{js_keys, js_order, quote, stringify};
use crate::protocol::LOG_V;
use crate::settle::{self, Js, JsError};

/// How many sequenced batches a room keeps, to answer a batch seen before.
const LOG_KEEP: usize = 2000;

/// An entry's `seq`, as the room counts.
pub fn seq_of(entry: &Value) -> f64 {
    entry.get("seq").and_then(Value::as_f64).unwrap_or(f64::NAN)
}

#[derive(Default)]
pub struct Room {
    pub head: f64,
    files: IndexMap<String, String>,
    log: VecDeque<Value>,
    pub conflicts: ConflictSet,
    bases: TextBases,
}

/// Why a submit did not order its batch: the throw the relay answers as an `error` frame.
#[derive(Debug)]
pub struct Refused(pub String);

impl From<JsError> for Refused {
    fn from(e: JsError) -> Self {
        Refused(e.message)
    }
}

impl Room {
    /// A room restored from what `state()` saved.
    pub fn from_state(saved: &Map<String, Value>) -> Room {
        let mut room = Room {
            head: saved
                .get("head")
                .and_then(Value::as_f64)
                .unwrap_or(f64::NAN),
            ..Room::default()
        };
        if let Some(Value::Object(files)) = saved.get("files") {
            for f in js_keys(files) {
                if let Some(t) = files[f].as_str() {
                    room.files.insert(f.clone(), t.to_string());
                }
            }
        }
        room.log = saved
            .get("log")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into();
        let conflicts = saved
            .get("conflicts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        room.conflicts.reset(&conflicts);
        room.bases = TextBases::from_saved(saved.get("bases"));
        room
    }

    pub fn files(&self) -> &IndexMap<String, String> {
        &self.files
    }

    /// `JSON.stringify({ v: LOG_V, ...room.state() })`, written without building the state twice.
    pub fn state_text(&self) -> String {
        let mut out = format!(
            "{{\"v\":{},\"head\":{},\"files\":",
            num(LOG_V),
            num(self.head)
        );
        out.push_str(&files_json(&self.files));
        out.push_str(",\"log\":[");
        for (i, e) in self.log.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&stringify(e));
        }
        out.push_str("],\"conflicts\":");
        out.push_str(&self.conflicts_json());
        out.push_str(",\"bases\":");
        let mut bases = Map::new();
        for (f, v) in self.bases.iter() {
            bases.insert(
                f.clone(),
                Value::Array(v.map(|t| Value::String(t.into())).collect()),
            );
        }
        out.push_str(&stringify(&Value::Object(bases)));
        out.push('}');
        out
    }

    pub fn conflicts_json(&self) -> String {
        stringify(&Value::Array(self.conflicts.list().cloned().collect()))
    }

    /// Order `batch`; a batch seen before answers its first ordering, one naming a path outside the workspace is refused.
    pub fn submit(
        &mut self,
        js: &dyn Js,
        batch: Map<String, Value>,
        persist: impl FnOnce(&Value) -> std::io::Result<()>,
    ) -> Result<(Value, bool), Refused> {
        let ops = batch
            .get("ops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for op in &ops {
            let file = op.get("file").and_then(Value::as_str).unwrap_or("");
            crate::path::assert_sync_path(file).map_err(Refused)?;
        }
        let id = batch.get("id");
        if let Some(known) = self.log.iter().find(|e| e.pointer("/batch/id") == id) {
            return Ok((known.clone(), false));
        }
        let seq = self.head + 1.0;
        let mut applied = Vec::new();
        let mut parked: Vec<Value> = Vec::new();
        let mut before: IndexMap<String, (Option<String>, Option<Vec<Version>>)> = IndexMap::new();
        let park = |parked: &mut Vec<Value>, op: Value, kept: Option<Option<String>>| {
            let mut c = Map::new();
            c.insert(
                "id".into(),
                format!("{}.{}", k7_js::number::to_string(seq), parked.len()).into(),
            );
            c.insert("seq".into(), json!(seq));
            for k in ["replica", "ts", "label"] {
                if let Some(v) = batch.get(k) {
                    c.insert(k.into(), v.clone());
                }
            }
            c.insert("op".into(), op);
            if let Some(kept) = kept {
                c.insert(
                    "kept".into(),
                    kept.map(Value::String).unwrap_or(Value::Null),
                );
            }
            parked.push(Value::Object(c));
        };
        let result = (|| -> Result<(), JsError> {
            for op in ops {
                let file = op
                    .get("file")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let current = self.files.get(&file).cloned();
                let s = settle::settle(js, current.as_deref(), &op, &self.bases)?;
                for (a, kept) in s.aside {
                    park(&mut parked, a, kept);
                }
                let Some(apply) = s.apply else { continue };
                match s.next {
                    Ok(next) => {
                        before
                            .entry(file.clone())
                            .or_insert_with(|| (current, self.bases.snapshot(&file)));
                        self.write(&file, next);
                        applied.push(apply);
                    }
                    Err(()) => park(&mut parked, apply, None),
                }
            }
            Ok(())
        })();
        let undo =
            |room: &mut Room, before: IndexMap<String, (Option<String>, Option<Vec<Version>>)>| {
                for (file, (text, bases)) in before {
                    match text {
                        None => {
                            room.files.shift_remove(&file);
                        }
                        Some(t) => {
                            room.files.insert(file.clone(), t);
                        }
                    }
                    room.bases.restore(&file, bases);
                }
            };
        if let Err(e) = result {
            undo(self, before);
            return Err(e.into());
        }
        let mut b = batch;
        b.insert("ops".into(), Value::Array(applied));
        let entry = json!({ "seq": seq, "batch": Value::Object(b), "parked": parked });
        if let Err(e) = persist(&entry) {
            undo(self, before);
            return Err(Refused(e.to_string()));
        }
        self.record(entry.clone());
        Ok((entry, true))
    }

    /// Re-apply an entry this room ordered before a restart, from the log written beside its saved state.
    pub fn replay(&mut self, js: &dyn Js, entry: Value) -> Result<(), JsError> {
        if seq_of(&entry) <= self.head {
            return Ok(());
        }
        let ops = entry
            .pointer("/batch/ops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for op in &ops {
            let file = op
                .get("file")
                .map(|f| crate::protocol::js_string(Some(f)))
                .unwrap_or_default();
            let next = settle::apply(js, self.files.get(&file).map(String::as_str), op)?;
            self.write(&file, next);
        }
        self.record(entry);
        Ok(())
    }

    fn write(&mut self, file: &str, text: Option<String>) {
        self.bases.record(file, text.as_deref());
        match text {
            None => {
                self.files.shift_remove(file);
            }
            Some(t) => {
                self.files.insert(file.to_string(), t);
            }
        }
    }

    fn record(&mut self, entry: Value) {
        self.head = seq_of(&entry);
        self.conflicts.record(&entry);
        self.log.push_back(entry);
        while self.log.len() > LOG_KEEP {
            self.log.pop_front();
        }
    }
}

/// `String(n)` of a number.
pub fn num(n: f64) -> String {
    k7_js::number::to_string(n)
}

/// `JSON.stringify(Object.fromEntries(files))`.
pub fn files_json(files: &IndexMap<String, String>) -> String {
    let mut out = String::from("{");
    for (i, f) in js_order(files.keys()).into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote(f, &mut out);
        out.push(':');
        quote(&files[f.as_str()], &mut out);
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settle::fake::FakeJs;
    use k7_js::number::text_hash;

    fn batch(id: &str, ops: Value) -> Map<String, Value> {
        json!({ "id": id, "replica": "r", "local": 1, "ts": 5, "label": "edit", "ops": ops })
            .as_object()
            .cloned()
            .unwrap()
    }

    #[test]
    fn orders_sets_aside_and_answers_a_batch_seen_before() {
        let mut room = Room::default();
        let one = batch(
            "b1",
            json!([{ "kind": "text", "file": "a.md", "text": "one\n", "was": null }]),
        );
        let (e, fresh) = room.submit(&FakeJs, one.clone(), |_| Ok(())).unwrap();
        assert!(fresh);
        assert_eq!(
            stringify(&e),
            r#"{"seq":1,"batch":{"id":"b1","replica":"r","local":1,"ts":5,"label":"edit","ops":[{"kind":"text","file":"a.md","text":"one\n","was":null}]},"parked":[]}"#
        );
        assert!(!room.submit(&FakeJs, one, |_| Ok(())).unwrap().1);
        let late = batch(
            "b2",
            json!([{ "kind": "remove", "file": "a.md", "was": text_hash("zero\n") }]),
        );
        let (e, _) = room.submit(&FakeJs, late, |_| Ok(())).unwrap();
        assert_eq!(e["parked"][0]["id"], "2.0");
        assert_eq!(stringify(&e["parked"][0]), r#"{"id":"2.0","seq":2,"replica":"r","ts":5,"label":"edit","op":{"kind":"remove","file":"a.md","was":"#.to_string() + &stringify(&json!(text_hash("zero\n"))) + "}}");
        assert_eq!(room.conflicts.len(), 1);
    }

    #[test]
    fn a_failed_persist_leaves_the_room_as_it_was() {
        let mut room = Room::default();
        let b = batch(
            "b1",
            json!([{ "kind": "text", "file": "a.md", "text": "one\n" }]),
        );
        assert!(
            room.submit(&FakeJs, b.clone(), |_| Err(std::io::Error::other("disk")))
                .is_err()
        );
        assert!(room.files().is_empty());
        assert_eq!(room.head, 0.0);
        assert!(room.submit(&FakeJs, b, |_| Ok(())).unwrap().1);
        assert_eq!(room.head, 1.0);
    }

    #[test]
    fn refuses_a_path_outside_the_workspace() {
        let mut room = Room::default();
        let b = batch(
            "b1",
            json!([{ "kind": "text", "file": "../a", "text": "x" }]),
        );
        let e = room.submit(&FakeJs, b, |_| Ok(())).unwrap_err();
        assert_eq!(e.0, "refused sync path \"../a\": '..' segment");
    }

    #[test]
    fn its_state_round_trips() {
        let mut room = Room::default();
        let b = batch(
            "b1",
            json!([{ "kind": "text", "file": "2", "text": "x\n" }, { "kind": "text", "file": "a.md", "text": "y\n" }]),
        );
        room.submit(&FakeJs, b, |_| Ok(())).unwrap();
        let text = room.state_text();
        let v: Value = serde_json::from_str(&text).unwrap();
        let mut saved = v.as_object().cloned().unwrap();
        saved.shift_remove("v");
        assert_eq!(Room::from_state(&saved).state_text(), text);
        assert!(text.starts_with(r#"{"v":1,"head":1,"files":{"2":"x\n","a.md":"y\n"},"log":["#));
        assert!(text.ends_with(r#""conflicts":[],"bases":{"2":["x\n"],"a.md":["y\n"]}}"#));
    }
}
