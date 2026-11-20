//! mine.rs — what this device put in the list, by workspace name: `<name> -> id`, in a file of the account's folder.
//! What a reader gets wrong: this is how a removal is noticed (a name here whose folder is gone) and where a
//! workspace with no remote keeps its random id; it is per device, since only a remote can match a workspace across devices.

use std::collections::BTreeMap;
use std::path::Path;

use crate::entry::{self, Entry};
use crate::list;
use crate::store::ListStore;

pub const FILE: &str = ".k7-roam.json";

/// A workspace of this device: its folder name and its remote when connected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Local {
    pub name: String,
    pub remote: Option<String>,
}

pub fn load(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, mine: &BTreeMap<String, String>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(mine).unwrap_or_default())?;
    std::fs::rename(tmp, dir.join(FILE))
}

/// The id a workspace has now: its remote's, else the one kept here, else a new random one.
pub fn id_of(local: &Local, mine: &BTreeMap<String, String>) -> String {
    match &local.remote {
        Some(r) => entry::id_of_remote(r),
        None => mine
            .get(&local.name)
            .filter(|i| i.starts_with('L'))
            .cloned()
            .unwrap_or_else(entry::random_id),
    }
}

/// Brings the list and `mine` up to what is on this device; true when `mine` changed.
/// An id that changed (a workspace just connected) leaves a tombstone on its old key; a name no longer here, on its key.
pub fn sweep(
    store: &dyn ListStore,
    mine: &mut BTreeMap<String, String>,
    here: &[Local],
    now: u64,
) -> bool {
    let before = mine.clone();
    let mut next = BTreeMap::new();
    for w in here {
        let id = id_of(w, mine);
        if let Some(old) = mine.get(&w.name).filter(|o| **o != id) {
            list::forget(store, old, now);
        }
        list::keep(
            store,
            &Entry {
                id: id.clone(),
                name: w.name.clone(),
                remote: w.remote.clone(),
                added_at: now,
            },
        );
        next.insert(w.name.clone(), id);
    }
    for (name, id) in mine.iter().filter(|(n, _)| !next.contains_key(*n)) {
        let _ = name;
        // Another workspace of this device may hold the id (a rename of a connected one), and then it stays.
        if !next.values().any(|i| i == id) {
            list::forget(store, id, now);
        }
    }
    list::prune(store, now);
    *mine = next;
    *mine != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Item;
    use crate::store::Fake;

    fn local(name: &str, remote: Option<&str>) -> Local {
        Local {
            name: name.into(),
            remote: remote.map(str::to_string),
        }
    }

    #[test]
    fn a_new_workspace_is_listed_and_a_connected_one_carries_its_remote() {
        let s = Fake::default();
        let mut mine = BTreeMap::new();
        assert!(sweep(
            &s,
            &mut mine,
            &[local("A", None), local("B", Some("git@h:o/b.git"))],
            5
        ));
        let got = list::live(&s);
        assert_eq!(got.len(), 2);
        assert_eq!(mine["B"], entry::id_of_remote("git@h:o/b.git"));
        assert!(got.iter().any(|e| e.name == "A" && e.remote.is_none()));
        assert!(!sweep(
            &s,
            &mut mine,
            &[local("A", None), local("B", Some("git@h:o/b.git"))],
            6
        ));
    }

    #[test]
    fn a_workspace_gone_from_the_device_leaves_a_tombstone() {
        let s = Fake::default();
        let mut mine = BTreeMap::new();
        sweep(&s, &mut mine, &[local("A", None), local("B", None)], 5);
        sweep(&s, &mut mine, &[local("A", None)], 6);
        assert_eq!(list::live(&s).len(), 1);
        assert_eq!(list::items(&s).len(), 2);
        assert_eq!(mine.len(), 1);
    }

    #[test]
    fn connecting_a_workspace_moves_it_from_its_random_id_to_its_remote_s() {
        let s = Fake::default();
        let mut mine = BTreeMap::new();
        sweep(&s, &mut mine, &[local("A", None)], 5);
        let random = mine["A"].clone();
        sweep(&s, &mut mine, &[local("A", Some("git@h:o/a"))], 6);
        let got = list::live(&s);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, entry::id_of_remote("git@h:o/a"));
        assert!(
            list::items(&s)
                .iter()
                .any(|i| i.id() == random && matches!(i, Item::Gone { .. }))
        );
    }

    #[test]
    fn two_devices_that_cloned_one_remote_share_a_key_and_a_removal_elsewhere_spares_the_other() {
        let s = Fake::default();
        let (mut a, mut b) = (BTreeMap::new(), BTreeMap::new());
        sweep(&s, &mut a, &[local("Band", Some("git@h:o/band.git"))], 5);
        sweep(&s, &mut b, &[local("Band", Some("ssh://git@h/o/band"))], 6);
        assert_eq!(list::live(&s).len(), 1);
        // Device A removes it; B still has it and its next start does not bring it back.
        sweep(&s, &mut a, &[], 7);
        sweep(&s, &mut b, &[local("Band", Some("ssh://git@h/o/band"))], 8);
        assert!(list::live(&s).is_empty());
    }

    #[test]
    fn a_workspace_without_a_remote_keeps_its_random_id_across_sweeps() {
        let s = Fake::default();
        let mut mine = BTreeMap::new();
        sweep(&s, &mut mine, &[local("A", None)], 5);
        let id = mine["A"].clone();
        sweep(&s, &mut mine, &[local("A", None)], 6);
        assert_eq!(mine["A"], id);
    }

    #[test]
    fn the_file_round_trips_and_a_bad_one_is_empty() {
        let d = std::env::temp_dir().join(format!("k7-roam-mine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(load(&d).is_empty());
        let m = BTreeMap::from([("A".to_string(), "L1".to_string())]);
        save(&d, &m).unwrap();
        assert_eq!(load(&d), m);
        std::fs::write(d.join(FILE), b"{").unwrap();
        assert!(load(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
