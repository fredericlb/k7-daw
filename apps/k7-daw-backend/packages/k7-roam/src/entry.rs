//! entry.rs — one workspace of the list, as the store holds it under the key `ws.<id>`: live, or a tombstone.
//! What a reader gets wrong: the id of a connected workspace comes from its remote, so two devices that cloned one
//! repository write the same key; a workspace with no remote has a random id, kept on the device that made it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The key prefix every workspace of the list has; nothing else in the store is read.
pub const PREFIX: &str = "ws.";

/// A tombstone is kept this long (milliseconds), then dropped by `list::prune`.
pub const TOMBSTONE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// The git ssh address when the workspace is connected.
    pub remote: Option<String>,
    #[serde(rename = "addedAt")]
    pub added_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Live(Entry),
    Gone { id: String, at: u64 },
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Live(e) => &e.id,
            Item::Gone { id, .. } => id,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Tomb {
    id: String,
    deleted: bool,
    at: u64,
}

pub fn key(id: &str) -> String {
    format!("{PREFIX}{id}")
}

pub fn encode(item: &Item) -> String {
    match item {
        Item::Live(e) => serde_json::to_string(e),
        Item::Gone { id, at } => serde_json::to_string(&Tomb {
            id: id.clone(),
            deleted: true,
            at: *at,
        }),
    }
    .unwrap_or_default()
}

/// A pair of the store as an item; a key that is not `ws.<id>`, a body that does not parse, or one whose id is another key's is none.
pub fn decode(key: &str, value: &str) -> Option<Item> {
    let id = key.strip_prefix(PREFIX).filter(|i| !i.is_empty())?;
    let v: serde_json::Value = serde_json::from_str(value).ok()?;
    let item = if v.get("deleted").and_then(|d| d.as_bool()) == Some(true) {
        let t: Tomb = serde_json::from_value(v).ok()?;
        Item::Gone { id: t.id, at: t.at }
    } else {
        let e: Entry = serde_json::from_value(v).ok()?;
        (!e.name.is_empty()).then_some(())?;
        Item::Live(e)
    };
    (item.id() == id).then_some(item)
}

/// The same address in every spelling a host takes: `git@h:o/r.git`, `ssh://git@h/o/r` and `ssh://git@h/o/r.git/`.
pub fn normal_remote(url: &str) -> String {
    let u = url.trim();
    let u = u.strip_prefix("ssh://").unwrap_or(u);
    let u = match (u.find(':'), u.find('/')) {
        (Some(c), s) if s.is_none_or(|s| c < s) => format!("{}/{}", &u[..c], &u[c + 1..]),
        _ => u.to_string(),
    };
    // The port is part of the address, so it stays; only the case of the host is folded.
    let (head, tail) = u.split_once('/').unwrap_or((u.as_str(), ""));
    let head = head.to_ascii_lowercase();
    let tail = tail.trim_end_matches('/');
    let tail = tail.strip_suffix(".git").unwrap_or(tail);
    format!("{head}/{tail}")
}

/// The id a connected workspace has on every device: a hash of its normalized remote.
pub fn id_of_remote(url: &str) -> String {
    let d = Sha256::digest(normal_remote(url).as_bytes());
    d[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// An id for a workspace with no remote: random, prefixed so it never meets a remote's.
pub fn random_id() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    format!(
        "L{}",
        b.iter().map(|x| format!("{x:02x}")).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(id: &str, remote: Option<&str>) -> Item {
        Item::Live(Entry {
            id: id.into(),
            name: "Band".into(),
            remote: remote.map(str::to_string),
            added_at: 7,
        })
    }

    #[test]
    fn an_entry_round_trips_with_its_remote_or_null() {
        for item in [live("a1", Some("git@h:o/r.git")), live("a2", None)] {
            let text = encode(&item);
            assert_eq!(decode(&key(item.id()), &text), Some(item));
        }
        assert!(encode(&live("a2", None)).contains(r#""remote":null"#));
        assert!(encode(&live("a2", None)).contains(r#""addedAt":7"#));
    }

    #[test]
    fn a_tombstone_round_trips_as_deleted_at() {
        let t = Item::Gone {
            id: "a1".into(),
            at: 9,
        };
        assert_eq!(encode(&t), r#"{"id":"a1","deleted":true,"at":9}"#);
        assert_eq!(decode("ws.a1", &encode(&t)), Some(t));
    }

    #[test]
    fn what_is_not_a_workspace_of_the_list_is_none() {
        assert_eq!(decode("other.a1", &encode(&live("a1", None))), None);
        assert_eq!(decode("ws.", "{}"), None);
        assert_eq!(decode("ws.a1", "not json"), None);
        assert_eq!(
            decode(
                "ws.a1",
                r#"{"id":"a1","name":"","remote":null,"addedAt":1}"#
            ),
            None
        );
        // A body filed under another id is another device's mistake, not an entry.
        assert_eq!(decode("ws.zz", &encode(&live("a1", None))), None);
    }

    #[test]
    fn every_spelling_of_a_remote_has_one_id() {
        let same = [
            "git@Host.example:org/repo.git",
            "ssh://git@host.example/org/repo",
            "ssh://git@host.example/org/repo.git/",
            "  git@host.example:org/repo  ",
        ];
        let ids: Vec<_> = same.iter().map(|u| id_of_remote(u)).collect();
        assert!(ids.windows(2).all(|w| w[0] == w[1]), "{ids:?}");
        assert_ne!(ids[0], id_of_remote("git@host.example:org/other.git"));
        assert_eq!(ids[0].len(), 16);
    }

    #[test]
    fn a_random_id_never_meets_a_remotes() {
        let (a, b) = (random_id(), random_id());
        assert_ne!(a, b);
        assert!(a.starts_with('L'));
    }
}
