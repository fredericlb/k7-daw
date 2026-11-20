//! list.rs — the rules of the list over a store: what a device writes, when a tombstone wins, and when it is dropped.
//! What a reader gets wrong: a device never writes a key it has no workspace for, except the tombstone of one it removed;
//! a tombstone stays over a workspace that still exists here, and only an explicit add or connect writes over it.

use crate::entry::{self, Entry, Item, TOMBSTONE_MS};
use crate::store::ListStore;

/// Every item of the store, in key order.
pub fn items(store: &dyn ListStore) -> Vec<Item> {
    store
        .pairs()
        .iter()
        .filter_map(|(k, v)| entry::decode(k, v))
        .collect()
}

/// The workspaces that are not removed.
pub fn live(store: &dyn ListStore) -> Vec<Entry> {
    items(store)
        .into_iter()
        .filter_map(|i| match i {
            Item::Live(e) => Some(e),
            Item::Gone { .. } => None,
        })
        .collect()
}

fn held(store: &dyn ListStore, id: &str) -> Option<Item> {
    store
        .pairs()
        .iter()
        .find(|(k, _)| *k == entry::key(id))
        .and_then(|(k, v)| entry::decode(k, v))
}

/// An add or a connect: the entry is written, over a tombstone too, and keeps the date the list first had it.
pub fn upsert(store: &dyn ListStore, e: &Entry) {
    let mut e = e.clone();
    if let Some(Item::Live(was)) = held(store, &e.id) {
        e.added_at = was.added_at;
    }
    store.put(&entry::key(&e.id), Some(&entry::encode(&Item::Live(e))));
}

/// What a start or a sweep writes: the entry only when the list has none or its name or remote differ, never over a tombstone.
pub fn keep(store: &dyn ListStore, e: &Entry) -> bool {
    match held(store, &e.id) {
        Some(Item::Gone { .. }) => false,
        Some(Item::Live(was)) if was.name == e.name && was.remote == e.remote => false,
        _ => {
            upsert(store, e);
            true
        }
    }
}

/// A removed workspace: its key holds a tombstone from now on.
pub fn forget(store: &dyn ListStore, id: &str, now: u64) {
    let gone = Item::Gone {
        id: id.to_string(),
        at: now,
    };
    store.put(&entry::key(id), Some(&entry::encode(&gone)));
}

/// Drops the tombstones older than their 30 days; returns how many.
pub fn prune(store: &dyn ListStore, now: u64) -> usize {
    let old: Vec<String> = items(store)
        .into_iter()
        .filter_map(|i| match i {
            Item::Gone { id, at } if now.saturating_sub(at) > TOMBSTONE_MS => Some(id),
            _ => None,
        })
        .collect();
    for id in &old {
        store.put(&entry::key(id), None);
    }
    old.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Fake;

    fn e(id: &str, remote: Option<&str>, at: u64) -> Entry {
        Entry {
            id: id.into(),
            name: "Band".into(),
            remote: remote.map(str::to_string),
            added_at: at,
        }
    }

    #[test]
    fn an_added_workspace_keeps_the_date_the_list_first_had_it() {
        let s = Fake::default();
        upsert(&s, &e("a", None, 5));
        upsert(&s, &e("a", Some("git@h:o/r"), 99));
        let got = live(&s);
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].added_at, got[0].remote.as_deref()),
            (5, Some("git@h:o/r"))
        );
    }

    #[test]
    fn a_tombstone_hides_the_entry_and_stays_over_a_start_s_sweep() {
        let s = Fake::default();
        upsert(&s, &e("a", Some("r"), 5));
        forget(&s, "a", 10);
        assert!(live(&s).is_empty());
        assert!(
            !keep(&s, &e("a", Some("r"), 20)),
            "a start never undoes a removal"
        );
        assert!(live(&s).is_empty());
        upsert(&s, &e("a", Some("r"), 30));
        assert_eq!(live(&s).len(), 1, "an explicit add does");
    }

    #[test]
    fn a_sweep_writes_only_what_is_missing_or_changed() {
        let s = Fake::default();
        assert!(keep(&s, &e("a", None, 1)));
        assert!(!keep(&s, &e("a", None, 2)));
        assert!(keep(&s, &e("a", Some("r"), 3)));
        assert_eq!(live(&s)[0].added_at, 1);
    }

    #[test]
    fn a_tombstone_is_dropped_after_thirty_days_and_not_before() {
        let s = Fake::default();
        forget(&s, "old", 0);
        forget(&s, "new", TOMBSTONE_MS);
        upsert(&s, &e("live", None, 0));
        assert_eq!(
            prune(&s, TOMBSTONE_MS),
            0,
            "exactly thirty days is still kept"
        );
        assert_eq!(prune(&s, TOMBSTONE_MS + 1), 1);
        let ids: Vec<_> = items(&s).iter().map(|i| i.id().to_string()).collect();
        assert_eq!(ids, ["live", "new"]);
    }

    #[test]
    fn keys_that_are_not_the_list_s_are_left_alone() {
        let s = Fake::default();
        s.put("other", Some("x"));
        s.put("ws.bad", Some("{"));
        forget(&s, "a", u64::MAX);
        prune(&s, u64::MAX);
        assert_eq!(s.pairs().len(), 3);
    }
}
