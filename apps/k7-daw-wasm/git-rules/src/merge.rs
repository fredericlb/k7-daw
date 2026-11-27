// merge.rs — a merge's conflicts and what each offers: one choice per conflict, and when "Finish merge" is allowed.
// A reader would get wrong that `Both` is offered only where the route says so (a keyed list item, a binary's copy beside).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// What the person picks for one conflict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "MergeChoice"))]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Mine,
    Theirs,
    /// Both texts (a block of lines, mine then theirs) or both files (theirs written beside under a free name).
    Both,
    /// One side removed a file the other changed: keep it.
    Keep,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ConflictKind"))]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    K7x,
    Text,
    Binary,
    Presence,
}

/// One thing the merge could not settle by itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Conflict {
    pub id: String,
    pub path: String,
    pub kind: Kind,
    /// The key path of a `.k7x` change, dotted.
    #[serde(default, rename = "keyPath", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub key_path: Option<String>,
    /// The first line of a text block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub line: Option<i64>,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub mine: Option<String>,
    #[serde(default)]
    pub theirs: Option<String>,
    /// Whether `Both` is offered.
    pub both: bool,
    #[serde(default)]
    pub choice: Option<Choice>,
}

/// The choices a conflict offers: a removed file keeps or goes; the rest take a side, and both where it is offered.
#[must_use]
pub fn offered(c: &Conflict) -> Vec<Choice> {
    if c.kind == Kind::Presence {
        vec![Choice::Keep, Choice::Remove]
    } else if c.both {
        vec![Choice::Mine, Choice::Theirs, Choice::Both]
    } else {
        vec![Choice::Mine, Choice::Theirs]
    }
}

/// Conflicts still without a choice; `chosen` holds the ids that have one.
#[must_use]
pub fn remaining(conflicts: &[Conflict], chosen: &HashSet<String>) -> usize {
    conflicts.iter().filter(|c| !chosen.contains(&c.id)).count()
}

/// Whether "Finish merge" is allowed: every conflict has a choice.
#[must_use]
pub fn can_finish(conflicts: &[Conflict], chosen: &HashSet<String>) -> bool {
    !conflicts.is_empty() && remaining(conflicts, chosen) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, kind: Kind, both: bool) -> Conflict {
        Conflict {
            id: id.into(),
            path: "song.k7x".into(),
            kind,
            key_path: None,
            line: None,
            base: None,
            mine: Some("1".into()),
            theirs: Some("2".into()),
            both,
            choice: None,
        }
    }

    #[test]
    fn offers_both_only_where_the_route_says_so() {
        assert_eq!(offered(&c("a", Kind::K7x, false)), [Choice::Mine, Choice::Theirs]);
        assert_eq!(
            offered(&c("a", Kind::K7x, true)),
            [Choice::Mine, Choice::Theirs, Choice::Both]
        );
    }

    #[test]
    fn offers_keep_or_remove_for_a_removed_file_never_a_side() {
        assert_eq!(offered(&c("a", Kind::Presence, false)), [Choice::Keep, Choice::Remove]);
        assert_eq!(offered(&c("a", Kind::Presence, true)), [Choice::Keep, Choice::Remove]);
    }

    #[test]
    fn waits_for_every_conflict() {
        let cs = [c("a", Kind::K7x, false), c("b", Kind::K7x, false)];
        let mut chosen = HashSet::new();
        assert!(!can_finish(&cs, &chosen));
        chosen.insert("a".to_owned());
        assert_eq!(remaining(&cs, &chosen), 1);
        assert!(!can_finish(&cs, &chosen));
        chosen.insert("b".to_owned());
        assert!(can_finish(&cs, &chosen));
        assert!(!can_finish(&[], &HashSet::new()));
    }
}
