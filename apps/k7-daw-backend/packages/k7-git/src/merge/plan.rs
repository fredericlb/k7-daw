//! plan.rs — what each path of the two sides did since their merge base, one case per path.
//! What a reader gets wrong: only paths that differ on both sides need a merge; a side that left a path as the
//! base had it simply yields to the other, and two sides that agree are no case at all.

use std::collections::BTreeMap;

use git2::{ObjectType, Oid, Repository, Tree, TreeWalkMode, TreeWalkResult};

pub type Files = BTreeMap<String, (Oid, u32)>;

/// Every file of a tree: its blob and mode.
pub fn files_of(tree: Option<&Tree<'_>>) -> Files {
    let mut out = Files::new();
    if let Some(t) = tree {
        let _ = t.walk(TreeWalkMode::PreOrder, |dir, e| {
            if e.kind() == Some(ObjectType::Blob) {
                out.insert(
                    format!("{dir}{}", e.name().unwrap_or("")),
                    (e.id(), e.filemode() as u32),
                );
            }
            TreeWalkResult::Ok
        });
    }
    out
}

pub fn tree_of<'r>(repo: &'r Repository, id: Option<Oid>) -> Option<Tree<'r>> {
    repo.find_commit(id?).ok()?.tree().ok()
}

/// What happened to one path.
#[derive(Debug, PartialEq, Eq)]
pub enum Case {
    /// Same on both sides (or untouched by either).
    Same,
    /// Only their side changed it: take it (`None` is a removal).
    Theirs(Option<(Oid, u32)>),
    /// Only this branch changed it: keep it.
    Mine,
    /// Both changed it, differently; a side removed it when its entry is none.
    Both {
        base: Option<(Oid, u32)>,
        mine: Option<(Oid, u32)>,
        theirs: Option<(Oid, u32)>,
    },
}

/// The case of one path from its three entries.
pub fn case_of(
    base: Option<(Oid, u32)>,
    mine: Option<(Oid, u32)>,
    theirs: Option<(Oid, u32)>,
) -> Case {
    if mine == theirs {
        return Case::Same;
    }
    if mine == base {
        return Case::Theirs(theirs);
    }
    if theirs == base {
        return Case::Mine;
    }
    Case::Both { base, mine, theirs }
}

/// Every path any side holds, sorted.
pub fn paths(a: &Files, b: &Files, c: &Files) -> Vec<String> {
    let mut all: Vec<String> = a.keys().chain(b.keys()).chain(c.keys()).cloned().collect();
    all.sort();
    all.dedup();
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(n: u8) -> Option<(Oid, u32)> {
        Some((Oid::from_bytes(&[n; 20]).unwrap(), 0o100644))
    }

    #[test]
    fn each_path_falls_in_one_case() {
        assert_eq!(case_of(e(1), e(1), e(1)), Case::Same);
        assert_eq!(
            case_of(e(1), e(2), e(2)),
            Case::Same,
            "both made the same change"
        );
        assert_eq!(case_of(e(1), e(1), e(2)), Case::Theirs(e(2)));
        assert_eq!(
            case_of(e(1), e(1), None),
            Case::Theirs(None),
            "they removed what I left alone"
        );
        assert_eq!(case_of(e(1), e(2), e(1)), Case::Mine);
        assert_eq!(
            case_of(None, None, e(3)),
            Case::Theirs(e(3)),
            "added on their side"
        );
        assert_eq!(
            case_of(e(1), e(2), e(3)),
            Case::Both {
                base: e(1),
                mine: e(2),
                theirs: e(3)
            }
        );
        assert_eq!(
            case_of(e(1), e(2), None),
            Case::Both {
                base: e(1),
                mine: e(2),
                theirs: None
            }
        );
        assert_eq!(
            case_of(None, e(2), e(3)),
            Case::Both {
                base: None,
                mine: e(2),
                theirs: e(3)
            },
            "added on both"
        );
    }
}
