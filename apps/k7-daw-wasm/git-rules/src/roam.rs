// roam.rs — the workspaces added on the person's other devices: what the list route answers and which this device offers to clone.
// A reader would get wrong that only an entry with a remote can come here, and that the by-name order is the browser's (Intl), not Rust's.

use serde::{Deserialize, Serialize};

/// One workspace of the list, as `GET /git/roam` answers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RoamEntry {
    pub id: String,
    pub name: String,
    /// The git ssh address, or null for a workspace that is not connected.
    pub remote: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub added_at: u64,
}

/// `available` is false on a host with no list store (anything but iOS); `here` holds this device's ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RoamAnswer {
    pub available: bool,
    pub entries: Vec<RoamEntry>,
    pub here: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RoamState {
    pub answer: Option<RoamAnswer>,
    /// Entries cloned from this section, hidden until a read says they are here.
    pub cloned: Vec<String>,
}

/// The entries to offer: connected, not on this device, not just cloned; the caller puts them by name.
#[must_use]
pub fn offered_roam(s: &RoamState) -> Vec<RoamEntry> {
    let Some(a) = s.answer.as_ref().filter(|a| a.available) else {
        return Vec::new();
    };
    a.entries
        .iter()
        .filter(|e| e.remote.is_some() && !a.here.contains(&e.id) && !s.cloned.contains(&e.id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, remote: Option<&str>) -> RoamEntry {
        RoamEntry {
            id: id.into(),
            name: name.into(),
            remote: remote.map(Into::into),
            added_at: 1,
        }
    }
    fn state(entries: Vec<RoamEntry>, here: &[&str], cloned: &[&str]) -> RoamState {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        RoamState {
            answer: Some(RoamAnswer {
                available: true,
                entries,
                here: own(here),
            }),
            cloned: own(cloned),
        }
    }

    #[test]
    fn lists_a_connected_workspace_this_device_lacks() {
        let s = state(
            vec![
                entry("b", "Zed", Some("git@h:o/z.git")),
                entry("a", "Band", Some("git@h:o/b.git")),
            ],
            &[],
            &[],
        );
        assert_eq!(offered_roam(&s).len(), 2);
    }

    #[test]
    fn never_lists_a_workspace_with_no_remote() {
        assert!(offered_roam(&state(vec![entry("a", "Local", None)], &[], &[])).is_empty());
    }

    #[test]
    fn never_lists_a_workspace_already_here_or_just_cloned() {
        let e = || vec![entry("a", "Band", Some("git@h:o/b.git"))];
        assert!(offered_roam(&state(e(), &["a"], &[])).is_empty());
        assert!(offered_roam(&state(e(), &[], &["a"])).is_empty());
    }

    #[test]
    fn lists_nothing_where_the_host_has_no_list() {
        let none = RoamState {
            answer: Some(RoamAnswer {
                available: false,
                entries: vec![],
                here: vec![],
            }),
            cloned: vec![],
        };
        assert!(offered_roam(&none).is_empty());
        assert!(
            offered_roam(&RoamState {
                answer: None,
                cloned: vec![]
            })
            .is_empty()
        );
    }
}
