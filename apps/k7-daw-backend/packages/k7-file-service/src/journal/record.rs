//! record.rs — a mutation bracketed by two images of the project, each from one walk, the difference journalled (1703, 1730
//! to 1739, 1742). What a reader gets wrong: the record holds the gate shared from `begin` to `end`, so an undo waits for it
//! and a record that begins while an undo waits queues behind it; `end` never fails a good write, it reports.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use serde_json::Value;
use tokio::sync::OwnedRwLockReadGuard;

use super::diff::{BinChange, changed_files, diff_bin, diff_text};
use super::format::EngineRef;
use super::image::{Image, OMap, image_of};
use super::ring::{Joined, NewBlob, NewEntry, Pushed, join, push};
use super::{Journals, Mode, Owner, Rec, Root, locked};
use crate::libraries::Home;
use crate::paths::join as join_path;
use crate::rel::to_rel;
use crate::revision::walk_project;
use crate::space::Space;

/// A file's `toRel`: the project's spelling and its path below it.
pub fn rel_of(prefix: &str, f: &crate::revision::Walked) -> String {
    if prefix == "." {
        f.rel.clone()
    } else {
        format!("{prefix}/{}", f.rel)
    }
}

/// What `begin` answers: the record's id and the token naming the instant before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Begun {
    pub id: u64,
    pub rev: String,
}

/// What `end` answers: the token after, the files that moved (`None` when no diff was taken), and the blobs to hand to an engine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ended {
    pub after: String,
    pub files: Option<Vec<Value>>,
    /// Blob copies stored here that the bound engine should keep; the copies go once it holds them (1741).
    pub keep: Vec<String>,
    /// The store those names are in.
    pub store: Option<String>,
    /// The record was abandoned before it ended: nothing was journalled.
    pub abandoned: bool,
    /// Why the journal failed after the write landed, logged and never answered (1763).
    pub failure: Option<String>,
}

/// A staged copy is dated now: `copy` keeps the source's date, which would read as a crash's leftover (1726).
fn fresh(file: &str) -> bool {
    std::fs::File::options()
        .write(true)
        .open(file)
        .and_then(|f| f.set_modified(std::time::SystemTime::now()))
        .is_ok()
}

pub(crate) fn unstage(rec: &Rec) {
    for (_, f) in rec.pre.iter() {
        let _ = std::fs::remove_file(f);
    }
}

fn say_once(root: &Root, key: String, line: String) {
    if locked(&root.state).said.insert(key) {
        eprintln!("{line}");
    }
}

impl Journals {
    /// The gate taken shared, after any undo waiting or running (1730): what `begin` waits for, for a caller that walks off its own thread.
    pub async fn enter(&self, root: &str) -> OwnedRwLockReadGuard<()> {
        self.root(root).gate.clone().read_owned().await
    }

    /// The text bytes the root's last image held: what each record in flight keeps a copy of (1780a).
    pub fn image_bytes(&self, root: &str) -> usize {
        self.root(root).image_bytes.load(Ordering::Relaxed)
    }

    /// Begin a record: wait for any undo, then take the gate shared and the "before" image (1730).
    pub async fn begin(
        &self,
        home: &Home,
        root: &str,
        dir: Option<&str>,
        label: &str,
        mode: Mode,
        owner: Owner,
    ) -> Begun {
        let guard = self.enter(root).await;
        self.begin_with(guard, home, root, dir, label, mode, owner)
    }

    /// The part of `begin` after the gate: the walk, the image and the token.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_with(
        &self,
        guard: OwnedRwLockReadGuard<()>,
        home: &Home,
        root: &str,
        dir: Option<&str>,
        label: &str,
        mode: Mode,
        owner: Owner,
    ) -> Begun {
        let r = self.root(root);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        r.begun.fetch_add(1, Ordering::SeqCst);
        let (before, rev) = match dir {
            None => (None, self.revisions.token(home, root, None, false)),
            Some(d) => match self.image(&r, home, root, d) {
                Some((img, rev)) => (Some(img), rev),
                None => (None, self.revisions.token(home, root, Some(d), true)),
            },
        };
        let rec = Rec {
            _guard: guard,
            home: home.clone(),
            root: root.to_string(),
            dir: dir.map(String::from),
            label: label.to_string(),
            mode,
            owner,
            before,
            pre: OMap::default(),
            staged: 0,
        };
        locked(&r.state).records.insert(id, rec);
        locked(&self.by_id).insert(id, r);
        Begun { id, rev }
    }

    /// The project at one instant from one walk: the image and the token naming it (1731); `None` past a limit (1739).
    fn image(&self, r: &Root, home: &Home, root: &str, dir: &str) -> Option<(Image, String)> {
        let walk = walk_project(dir, &self.limits, false);
        if walk.reached {
            say_once(
                r,
                format!("limit {root}"),
                format!(
                    "[files] native: journal skipped a record of {root}: the project is past the walk's limits"
                ),
            );
            return None;
        }
        let space = Space::open(home, root);
        // Every file below the project answers to the project's own spelling, so it is asked once.
        let prefix = to_rel(&space, dir);
        let now = (self.clock)() as f64;
        let img = image_of(
            &walk.files,
            &|f| rel_of(&prefix, f),
            &mut locked(&r.text),
            now,
            self.limits.max_text,
            &mut |p| std::fs::read(p),
        );
        match img {
            Ok(img) => {
                let bytes = img.text.iter().map(|(_, t)| t.len()).sum();
                r.image_bytes.store(bytes, Ordering::Relaxed);
                Some((
                    img,
                    self.revisions.walked(home, root, dir, walk.summary, false),
                ))
            }
            Err(e) => {
                say_once(
                    r,
                    format!("large {root}"),
                    format!(
                        "[files] native: journal skipped a record of {root}: {} is past the text limit",
                        e.0
                    ),
                );
                None
            }
        }
    }

    /// The project's image now: nothing is recorded, staged or held (1711).
    pub fn image_now(&self, home: &Home, root: &str, dir: &str) -> Option<Image> {
        self.image(&self.root(root), home, root, dir)
            .map(|(i, _)| i)
    }

    /// Stage a copy of the file at `abs` before a mutation overwrites it (1742); never held in memory.
    pub fn snapshot(&self, id: u64, abs: &str) {
        let Some(r) = locked(&self.by_id).get(&id).cloned() else {
            return;
        };
        let (rel, to, store) = {
            let mut st = locked(&r.state);
            let Some(rec) = st.records.get_mut(&id) else {
                return;
            };
            let Some(dir) = rec.dir.clone() else { return };
            let rel = to_rel(&Space::open(&rec.home, &rec.root), abs);
            if rec.pre.contains(&rel) {
                return;
            }
            rec.staged += 1;
            let store = super::store::Store::new(super::store::store_for(&rec.home, &dir));
            (
                rel,
                join_path(&[&store.staged_dir(), &format!("{id}-{}", rec.staged)]),
                store,
            )
        };
        // Best effort: a missed pre-image only means that one file cannot be undone.
        let ok = std::fs::metadata(abs).is_ok_and(|m| m.is_file())
            && std::fs::create_dir_all(store.staged_dir()).is_ok()
            && std::fs::copy(abs, &to).is_ok()
            && fresh(&to);
        if !ok {
            let _ = std::fs::remove_file(&to);
            return;
        }
        match locked(&r.state).records.get_mut(&id) {
            Some(rec) => rec.pre.set(rel, to),
            None => {
                let _ = std::fs::remove_file(&to);
            }
        }
    }

    /// The mutation threw: nothing is journalled, the gate is let go.
    pub fn abort(&self, id: u64) {
        let Some(r) = locked(&self.by_id).remove(&id) else {
            return;
        };
        if let Some(rec) = locked(&r.state).records.remove(&id) {
            unstage(&rec);
        }
    }

    pub async fn end(&self, id: u64, engine: Option<EngineRef>) -> Ended {
        self.end_now(id, engine)
    }

    /// The mutation ran: the "after" image, the diff, the step pushed, the others rebased (1737).
    pub fn end_now(&self, id: u64, engine: Option<EngineRef>) -> Ended {
        let Some(r) = locked(&self.by_id).remove(&id) else {
            return Ended {
                abandoned: true,
                ..Ended::default()
            };
        };
        // Held to the push below, so a record is never out of `records` while another rebases (docs/file-journal.md).
        let _landing = locked(&r.landing);
        let Some(rec) = locked(&r.state).records.remove(&id) else {
            return Ended {
                abandoned: true,
                ..Ended::default()
            };
        };
        let (home, root) = (rec.home.clone(), rec.root.clone());
        let (Some(dir), Some(before)) = (rec.dir.clone(), rec.before.as_ref()) else {
            // No project, or a walk past its limits: fresh tokens, nothing diffed.
            let after = match &rec.dir {
                Some(d) => self.revisions.token(&home, &root, Some(d), true),
                None => self.revisions.token(&home, &root, None, false),
            };
            unstage(&rec);
            return Ended {
                after,
                ..Ended::default()
            };
        };
        let Some((after, rev_after)) = self.image(&r, &home, &root, &dir) else {
            unstage(&rec);
            let after = self.revisions.token(&home, &root, Some(&dir), true);
            return Ended {
                after,
                ..Ended::default()
            };
        };
        let space = Space::open(&home, &root);
        let store = super::store::Store::new(super::store::store_for(&home, &dir));
        let mut n = 0;
        let mut stage_after = |rel: &str| -> Option<String> {
            n += 1;
            let to = join_path(&[&store.staged_dir(), &format!("{id}-a{n}")]);
            std::fs::create_dir_all(store.staged_dir()).ok()?;
            let mut src = std::fs::File::open(join_path(&[&root, rel])).ok()?;
            let mut out = std::fs::File::create(&to).ok()?;
            std::io::copy(&mut src, &mut out).ok()?;
            Some(to)
        };
        let changes = diff_text(&before.text, &after.text);
        let mut stale = Vec::new();
        let blobs = diff_bin(
            &before.bin,
            &after.bin,
            &rec.pre,
            &mut stage_after,
            &mut |_, a| stale.push(a.to_string()),
        );
        for f in stale {
            let _ = std::fs::remove_file(f);
        }
        let shapes: Vec<(&str, bool, bool)> = changes
            .iter()
            .map(|c| (c.path.as_str(), c.before.is_none(), c.after.is_none()))
            .chain(
                blobs
                    .iter()
                    .map(|b| (b.path.as_str(), b.before.is_none(), b.after.is_none())),
            )
            .collect();
        let files = Some(changed_files(&to_rel(&space, &dir), &shapes));
        let mut out = Ended {
            after: rev_after,
            files,
            store: Some(store.dir.clone()),
            ..Ended::default()
        };
        let record = !(changes.is_empty() && blobs.is_empty()) && rec.mode != Mode::Skip;
        let mut st = locked(&r.state);
        rebase(&mut st.records, &changes, &blobs, &after);
        if record {
            let entry = NewEntry {
                label: rec.label.clone(),
                changes,
                blobs: blobs
                    .iter()
                    .map(|b| NewBlob {
                        path: b.path.clone(),
                        before: b.before.is_some(),
                        after: b.after.is_some(),
                    })
                    .collect(),
            };
            match self.land(
                &mut st,
                &home,
                &dir,
                entry,
                &blobs,
                rec.mode,
                engine.clone(),
            ) {
                Ok(keep) => out.keep = if engine.is_some() { keep } else { vec![] },
                Err(why) => {
                    out.failure = Some(why);
                    for b in &blobs {
                        b.before.iter().chain(&b.after).for_each(|f| {
                            let _ = std::fs::remove_file(f);
                        });
                    }
                }
            }
        } else {
            for b in &blobs {
                b.after.iter().for_each(|f| {
                    let _ = std::fs::remove_file(f);
                });
            }
        }
        drop(st);
        unstage(&rec);
        out
    }

    /// The step pushed into the journal of `dir`: blobs stored, then `journal.json` persisted (1751).
    #[allow(clippy::too_many_arguments)]
    fn land(
        &self,
        st: &mut super::State,
        home: &Home,
        dir: &str,
        entry: NewEntry,
        bins: &[BinChange],
        mode: Mode,
        engine: Option<EngineRef>,
    ) -> Result<Vec<String>, String> {
        let result = self.land_in(st, home, dir, entry, bins, mode, engine);
        // A failure leaves nothing in memory the disk lacks: the next use reads the journal again.
        if result.is_err() {
            st.open = None;
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn land_in(
        &self,
        st: &mut super::State,
        home: &Home,
        dir: &str,
        entry: NewEntry,
        bins: &[BinChange],
        mode: Mode,
        engine: Option<EngineRef>,
    ) -> Result<Vec<String>, String> {
        Self::open_in(st, home, dir);
        let open = st.open.as_mut().expect("opened");
        if let Some(why) = &open.held.unread {
            return Err(format!("journal did not read: {why}"));
        }
        let now = (self.clock)();
        let landed = if mode == Mode::Join {
            join(&mut open.held.journal, entry, now, self.max_entries, engine)
        } else {
            push(&mut open.held.journal, entry, now, self.max_entries, engine)
                .map_or(Joined::Nothing, Joined::Pushed)
        };
        let pushed: Option<Pushed> = match landed {
            Joined::Pushed(p) => Some(p),
            _ => None,
        };
        let mut keep = Vec::new();
        if let Some(p) = &pushed {
            let mut moves = Vec::new();
            for (b, (before, after)) in bins.iter().zip(&p.stored) {
                if let (Some(f), Some(n)) = (&b.before, before) {
                    moves.push((f.clone(), n.clone()));
                }
                if let (Some(f), Some(n)) = (&b.after, after) {
                    moves.push((f.clone(), n.clone()));
                }
            }
            keep = moves.iter().map(|(_, n)| n.clone()).collect();
            super::blobs::store_staged(&open.store, &moves).map_err(|e| e.to_string())?;
        }
        let dropped = pushed.map(|p| p.dropped).unwrap_or_default();
        let store = open.store.clone();
        self.save(st)?;
        super::blobs::delete(&store, &dropped);
        Ok(keep)
    }
}

/// What one landed record wrote becomes the starting point of every record still running (1737).
fn rebase(
    records: &mut HashMap<u64, Rec>,
    changes: &[super::format::Change],
    blobs: &[BinChange],
    after: &Image,
) {
    for other in records.values_mut() {
        let Some(img) = other.before.as_mut() else {
            continue;
        };
        for c in changes {
            match &c.after {
                None => {
                    img.text.remove(&c.path);
                }
                Some(t) => img.text.set(c.path.clone(), t.clone()),
            }
        }
        for b in blobs {
            match (after.bin.get(&b.path), &b.after) {
                (Some(stamp), Some(file)) => {
                    img.bin.set(b.path.clone(), *stamp);
                    other.staged += 1;
                    let to = format!("{file}.pre{}", other.staged);
                    if std::fs::hard_link(file, &to)
                        .or_else(|_| {
                            std::fs::copy(file, &to).map(|_| {
                                fresh(&to);
                            })
                        })
                        .is_ok()
                    {
                        if let Some(old) = other.pre.remove(&b.path) {
                            let _ = std::fs::remove_file(old);
                        }
                        other.pre.set(b.path.clone(), to);
                    }
                }
                _ => {
                    img.bin.remove(&b.path);
                    if let Some(old) = other.pre.remove(&b.path) {
                        let _ = std::fs::remove_file(old);
                    }
                }
            }
        }
    }
}
