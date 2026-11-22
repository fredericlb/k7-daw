//! forges.rs — the Forgejo servers a person added, one key per server (`forge.<id>`), beside the workspace list.
//! What a reader gets wrong: the device's own file is a copy, and a server it holds that the store has never seen is
//! pushed to the store, while one the store holds a tombstone for is gone from the device too (`docs/roam.md`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entry::TOMBSTONE_MS;
use crate::store::ListStore;

pub const PREFIX: &str = "forge.";

pub fn key(id: &str) -> String {
    format!("{PREFIX}{id}")
}

#[derive(Serialize, Deserialize)]
struct Tomb {
    id: String,
    deleted: bool,
    at: u64,
}

/// What the store holds for one server: its JSON body, or its removal.
#[derive(Clone, Debug, PartialEq)]
enum Held {
    Live(Value),
    Gone(u64),
}

fn decode(key: &str, text: &str) -> Option<(String, Held)> {
    let id = key.strip_prefix(PREFIX).filter(|i| !i.is_empty())?;
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("id").and_then(Value::as_str) != Some(id) {
        return None;
    }
    let held = if v.get("deleted").and_then(Value::as_bool) == Some(true) {
        Held::Gone(v.get("at").and_then(Value::as_u64)?)
    } else {
        Held::Live(v)
    };
    Some((id.to_string(), held))
}

fn held(store: &dyn ListStore) -> Vec<(String, Held)> {
    store
        .pairs()
        .iter()
        .filter_map(|(k, v)| decode(k, v))
        .collect()
}

/// An add: the server's body under its key, over a tombstone too; `server` carries its own `id`.
pub fn put(store: &dyn ListStore, id: &str, server: &Value) {
    store.put(&key(id), Some(&server.to_string()));
}

/// A removal: a tombstone, so a device that still lists the server drops it instead of bringing it back.
pub fn forget(store: &dyn ListStore, id: &str, now: u64) {
    let t = Tomb {
        id: id.to_string(),
        deleted: true,
        at: now,
    };
    store.put(&key(id), serde_json::to_string(&t).ok().as_deref());
}

/// The servers of every device, from the store and this device's copy, in the copy's order then the store's.
/// A server only here is pushed; a body the store holds wins over the copy's; a tombstone drops it here.
pub fn merge(store: &dyn ListStore, here: &[Value], now: u64) -> Vec<Value> {
    let kept = held(store);
    let find = |id: &str| kept.iter().find(|(i, _)| i == id).map(|(_, h)| h);
    let id_of = |v: &Value| v.get("id").and_then(Value::as_str).map(str::to_string);
    let mut out: Vec<Value> = Vec::new();
    for v in here {
        let Some(id) = id_of(v) else { continue };
        match find(&id) {
            Some(Held::Live(theirs)) => out.push(theirs.clone()),
            Some(Held::Gone(_)) => {}
            None => {
                put(store, &id, v);
                out.push(v.clone());
            }
        }
    }
    for (id, h) in &kept {
        match h {
            Held::Live(v) if !out.iter().any(|o| id_of(o).as_deref() == Some(id)) => {
                out.push(v.clone())
            }
            Held::Gone(at) if now.saturating_sub(*at) > TOMBSTONE_MS => store.put(&key(id), None),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Fake;
    use serde_json::json;

    fn server(id: &str, client: &str) -> Value {
        json!({ "id": id, "url": format!("https://{id}"), "clientId": client })
    }

    fn ids(v: &[Value]) -> Vec<&str> {
        v.iter().map(|s| s["id"].as_str().unwrap()).collect()
    }

    #[test]
    fn a_server_only_on_this_device_is_pushed_and_another_device_s_is_added() {
        let s = Fake::default();
        put(&s, "b.example", &server("b.example", "cb"));
        let got = merge(&s, &[server("a.example", "ca")], 1);
        assert_eq!(ids(&got), ["a.example", "b.example"]);
        assert_eq!(
            s.pairs().len(),
            2,
            "the device's own server is in the store now"
        );
    }

    #[test]
    fn the_store_s_body_wins_over_the_copy_s() {
        let s = Fake::default();
        put(&s, "a.example", &server("a.example", "new"));
        let got = merge(&s, &[server("a.example", "old")], 1);
        assert_eq!(got, [server("a.example", "new")]);
    }

    #[test]
    fn a_removal_elsewhere_drops_the_server_here_and_an_add_writes_over_it() {
        let s = Fake::default();
        forget(&s, "a.example", 5);
        assert!(merge(&s, &[server("a.example", "ca")], 6).is_empty());
        put(&s, "a.example", &server("a.example", "ca"));
        assert_eq!(merge(&s, &[], 7).len(), 1);
    }

    #[test]
    fn a_tombstone_goes_after_thirty_days_and_other_keys_are_left_alone() {
        let s = Fake::default();
        s.put("ws.x", Some("{}"));
        s.put("forge.bad", Some("{"));
        s.put("forge.c", Some(&server("other", "x").to_string()));
        forget(&s, "a.example", 0);
        merge(&s, &[], TOMBSTONE_MS);
        assert_eq!(s.pairs().len(), 4, "exactly thirty days is still kept");
        merge(&s, &[], TOMBSTONE_MS + 1);
        assert_eq!(s.pairs().len(), 3);
        assert!(
            merge(&s, &[], TOMBSTONE_MS + 1).is_empty(),
            "a body under another id is not a server"
        );
    }
}
