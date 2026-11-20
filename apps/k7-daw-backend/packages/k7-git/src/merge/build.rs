//! build.rs — the plan of a merge: each path's case settled into a result, a text to merge, or a conflict.
//! What a reader gets wrong: nothing here writes the work tree or moves `HEAD`; blobs of merged text are written
//! to the object database (so the pending file can name them) and are garbage if the merge is aborted.

use git2::Oid;

use super::pending::{Entry, Pending, Plan, Summary};
use super::plan::{Case, case_of, files_of, paths, tree_of};
use super::text::{Merged, merge_k7x, merge_lines};
use super::{Begin, Conflict, Kind};
use crate::lfs::Pointer;
use crate::{Error, Repo, Result};

type Side = Option<(Oid, u32)>;

impl Repo {
    fn blob_text(&self, s: Side) -> Option<String> {
        let (id, _) = s?;
        String::from_utf8(self.git.find_blob(id).ok()?.content().to_vec()).ok()
    }

    fn is_large(&self, s: Side) -> bool {
        s.is_some_and(|(id, _)| self.pointer_blob(id).is_some())
    }

    /// The pointer a blob holds, judged by the blob itself and never by an attributes file (docs/git.md).
    pub(crate) fn pointer_blob(&self, id: Oid) -> Option<Pointer> {
        let (size, _) = self.git.odb().ok()?.read_header(id).ok()?;
        if size >= crate::lfs::pointer::MAX_POINTER {
            return None;
        }
        Pointer::parse(self.git.find_blob(id).ok()?.content())
    }

    fn is_text(&self, s: Side) -> bool {
        let Some((id, _)) = s else { return true };
        let Ok(b) = self.git.find_blob(id) else {
            return false;
        };
        !self.is_large(s)
            && std::str::from_utf8(b.content()).is_ok()
            && !crate::changes::looks_binary(b.content())
    }

    fn put_text(&self, t: &str) -> Result<String> {
        Ok(self.git.blob(t.as_bytes())?.to_string())
    }

    /// Plans the merge of `theirs` into this branch; a diverged one is written to the git folder.
    pub fn merge_begin(&self, theirs: &str) -> Result<Begin> {
        self.merge_begin_fetching(theirs, |_| Ok(()))
    }

    /// The same, with `fetch` given the large files the merge brings before anything is written: a failure leaves no merge.
    pub fn merge_begin_fetching(
        &self,
        theirs: &str,
        fetch: impl FnOnce(&[(String, Pointer)]) -> Result<()>,
    ) -> Result<Begin> {
        if self.merge_pending()?.is_some() {
            return Err(Error::MergeOpen);
        }
        if self.status()?.iter().any(|e| !e.too_large) {
            return Err(Error::Uncommitted);
        }
        let theirs_id = Oid::from_str(theirs).map_err(|e| Error::Git(e.message().into()))?;
        self.git.find_commit(theirs_id)?;
        let theirs_files = files_of(tree_of(&self.git, Some(theirs_id)).as_ref());
        crate::paths::check_all(theirs_files.keys().map(String::as_str))?;
        self.refuse_case_clash(theirs_files.keys().map(String::as_str))?;
        let Ok(head) = self.git.refname_to_id("HEAD") else {
            return Ok(Begin::FastForward {
                theirs: theirs.to_string(),
                head: None,
            });
        };
        if head == theirs_id || self.git.graph_descendant_of(head, theirs_id)? {
            return Ok(Begin::UpToDate);
        }
        if self.git.graph_descendant_of(theirs_id, head)? {
            return Ok(Begin::FastForward {
                theirs: theirs.to_string(),
                head: Some(head.to_string()),
            });
        }
        let base = self.git.merge_base(head, theirs_id).ok();
        let (b, m, t) = (
            files_of(tree_of(&self.git, base).as_ref()),
            files_of(tree_of(&self.git, Some(head)).as_ref()),
            files_of(tree_of(&self.git, Some(theirs_id)).as_ref()),
        );
        let mut entries = Vec::new();
        let mut conflicts: Vec<Conflict> = Vec::new();
        let mut lfs = Vec::new();
        let author = self
            .git
            .find_commit(theirs_id)?
            .author()
            .name()
            .unwrap_or("theirs")
            .to_string();
        for path in paths(&b, &m, &t) {
            let (bs, ms, ts) = (
                b.get(&path).copied(),
                m.get(&path).copied(),
                t.get(&path).copied(),
            );
            match case_of(bs, ms, ts) {
                Case::Same | Case::Mine => {}
                Case::Theirs(side) => {
                    if let Some((id, _)) = side
                        && let Some(p) =
                            self.is_large(side).then(|| self.pointer_blob(id)).flatten()
                    {
                        lfs.push((path.clone(), p));
                    }
                    let plan = Plan::Result {
                        blob: side.map(|(i, _)| i.to_string()),
                        mode: side.map_or(0o100644, |s| s.1),
                    };
                    entries.push(Entry { path, plan });
                }
                Case::Both { base, mine, theirs } => {
                    let (plan, cs) =
                        self.both_changed(&path, base, mine, theirs, &author, &mut lfs)?;
                    conflicts.extend(cs);
                    entries.push(Entry { path, plan });
                }
            }
        }
        let p = Pending {
            head: head.to_string(),
            theirs: theirs.to_string(),
            base: base.map(|b| b.to_string()),
            entries,
            conflicts,
        };
        fetch(&lfs)?;
        self.merge_save(&p)?;
        Ok(Begin::Merge(Summary {
            theirs: theirs.to_string(),
            conflicts: p.conflicts.clone(),
            lfs,
        }))
    }

    fn both_changed(
        &self,
        path: &str,
        base: Side,
        mine: Side,
        theirs: Side,
        author: &str,
        lfs: &mut Vec<(String, Pointer)>,
    ) -> Result<(Plan, Vec<Conflict>)> {
        let mode = mine.or(theirs).map_or(0o100644, |s| s.1);
        let id_of = |s: Side| s.map(|(i, _)| i.to_string());
        // One side removed what the other changed.
        if mine.is_none() || theirs.is_none() {
            let kept = mine.or(theirs);
            let c = Conflict {
                id: format!("{path}#0"),
                path: path.into(),
                kind: Kind::Presence,
                key_path: None,
                line: None,
                base: None,
                mine: mine.map(|_| "changed here".to_string()),
                theirs: theirs.map(|_| "changed there".to_string()),
                both: false,
                choice: None,
            };
            if let Some((id, _)) = theirs
                && let Some(p) = self
                    .is_large(theirs)
                    .then(|| self.pointer_blob(id))
                    .flatten()
            {
                lfs.push((path.to_string(), p));
            }
            return Ok((
                Plan::Presence {
                    kept: id_of(kept),
                    mode,
                },
                vec![c],
            ));
        }
        let texty = self.is_text(base) && self.is_text(mine) && self.is_text(theirs);
        if texty {
            let (b, m, t) = (
                self.blob_text(base).unwrap_or_default(),
                self.blob_text(mine).unwrap_or_default(),
                self.blob_text(theirs).unwrap_or_default(),
            );
            let kind = if path.ends_with(".k7x") {
                Kind::K7x
            } else {
                Kind::Text
            };
            let merged = if kind == Kind::K7x && base.is_some() {
                merge_k7x(path, &b, &m, &t, author)
            } else {
                None
            }
            .or_else(|| merge_lines(path, &b, &m, &t, Kind::Text));
            match merged {
                Some(Merged::Clean(text)) => {
                    return Ok((
                        Plan::Result {
                            blob: Some(self.put_text(&text)?),
                            mode,
                        },
                        vec![],
                    ));
                }
                Some(Merged::K7x {
                    start,
                    aside,
                    conflicts,
                }) => return Ok((Plan::K7x { start, mode, aside }, conflicts)),
                Some(Merged::Lines { segs, conflicts }) => {
                    return Ok((Plan::Text { segs, mode }, conflicts));
                }
                None => {}
            }
        }
        // A binary file, an LFS file (equal by its oid it is already settled), or a text too long to merge.
        if let Some((id, _)) = theirs
            && let Some(p) = self
                .is_large(theirs)
                .then(|| self.pointer_blob(id))
                .flatten()
        {
            lfs.push((path.to_string(), p));
        }
        let c = Conflict {
            id: format!("{path}#0"),
            path: path.into(),
            kind: Kind::Presence,
            key_path: None,
            line: None,
            base: None,
            mine: Some(self.describe(mine)),
            theirs: Some(self.describe(theirs)),
            both: true,
            choice: None,
        };
        let c = Conflict {
            kind: Kind::Binary,
            ..c
        };
        Ok((
            Plan::Whole {
                mine: id_of(mine),
                theirs: id_of(theirs),
                mode,
                author: author.to_string(),
            },
            vec![c],
        ))
    }

    fn describe(&self, s: Side) -> String {
        let Some((id, _)) = s else {
            return "removed".into();
        };
        match self.pointer_blob(id) {
            Some(p) => format!("{} bytes", p.size),
            None => format!(
                "{} bytes",
                self.git.find_blob(id).map(|b| b.size()).unwrap_or(0)
            ),
        }
    }
}
