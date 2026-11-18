//! assemble.rs — a path's final content from the plan and the choices made so far.
//! What a reader gets wrong: a file with a conflict not yet chosen is left out, not guessed; a `.k7x` that does not
//! parse once its choices are in is refused with the parser's line, so a half-merged project never reaches the disk.

use serde_json::Value;

use super::pending::{Pending, Plan, Seg};
use super::{Choice, Conflict};
use crate::{Error, Repo, Result};

/// A file's content once settled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Blob { id: String, mode: u32 },
    Bytes { bytes: Vec<u8>, mode: u32 },
    Gone,
}

#[derive(Debug, Default)]
pub struct Assembled {
    /// Each path settled, in plan order; an extra path is a file kept beside.
    pub files: Vec<(String, Content)>,
    /// Conflicts still waiting for a choice.
    pub waiting: usize,
}

fn choice_of<'a>(cs: &'a [Conflict], id: &str) -> Option<(&'a Conflict, Choice)> {
    let c = cs.iter().find(|c| c.id == id)?;
    Some((c, c.choice?))
}

/// The refusal of a `.k7x` that does not read.
fn parses(path: &str, text: &str) -> Result<()> {
    if !path.ends_with(".k7x") {
        return Ok(());
    }
    k7_sync::k7x::apply_k7x(text, &[])
        .map(drop)
        .map_err(|e| Error::Unparsable {
            path: path.to_string(),
            message: e.to_string(),
        })
}

/// `name.ext` beside: `name (author).ext`, with a number when that is taken.
fn beside(taken: &dyn Fn(&str) -> bool, path: &str, author: &str) -> String {
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(d, f)| (d, f));
    let (stem, ext) = file
        .rsplit_once('.')
        .map_or((file, String::new()), |(s, e)| (s, format!(".{e}")));
    let who: String = author
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '\n' | '\r'))
        .collect();
    let mut n = 1;
    loop {
        let tag = if n == 1 {
            who.clone()
        } else {
            format!("{who} {n}")
        };
        let name = format!("{stem} ({tag}){ext}");
        let full = if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        };
        if !taken(&full) {
            return full;
        }
        n += 1;
    }
}

impl Repo {
    pub(crate) fn assemble(&self, p: &Pending) -> Result<Assembled> {
        let mut out = Assembled::default();
        let existing: Vec<&str> = p.entries.iter().map(|e| e.path.as_str()).collect();
        for e in &p.entries {
            let path = &e.path;
            match &e.plan {
                Plan::Result { blob, mode } => out.files.push((
                    path.clone(),
                    blob.as_ref().map_or(Content::Gone, |id| Content::Blob {
                        id: id.clone(),
                        mode: *mode,
                    }),
                )),
                Plan::Presence { kept, mode } => {
                    match choice_of(&p.conflicts, &format!("{path}#0")) {
                        Some((_, Choice::Keep)) => out.files.push((
                            path.clone(),
                            kept.as_ref().map_or(Content::Gone, |id| Content::Blob {
                                id: id.clone(),
                                mode: *mode,
                            }),
                        )),
                        Some((_, _)) => out.files.push((path.clone(), Content::Gone)),
                        None => out.waiting += 1,
                    }
                }
                Plan::Whole {
                    mine,
                    theirs,
                    mode,
                    author,
                } => match choice_of(&p.conflicts, &format!("{path}#0")) {
                    None => out.waiting += 1,
                    Some((_, choice)) => {
                        let blob = |s: &Option<String>| {
                            s.as_ref().map_or(Content::Gone, |id| Content::Blob {
                                id: id.clone(),
                                mode: *mode,
                            })
                        };
                        match choice {
                            Choice::Theirs => out.files.push((path.clone(), blob(theirs))),
                            Choice::Both => {
                                out.files.push((path.clone(), blob(mine)));
                                let taken = |n: &str| {
                                    existing.contains(&n) || self.workdir.join(n).exists()
                                };
                                out.files.push((beside(&taken, path, author), blob(theirs)));
                            }
                            _ => out.files.push((path.clone(), blob(mine))),
                        }
                    }
                },
                Plan::K7x { start, mode, aside } => {
                    let mut text = start.clone();
                    let mut open = false;
                    for (id, sub) in aside {
                        match choice_of(&p.conflicts, id) {
                            None => open = true,
                            Some((_, ch @ (Choice::Theirs | Choice::Both))) => {
                                let mut one: Value = sub.clone();
                                let beside = one.as_object_mut().and_then(|m| m.remove("both"));
                                if let (Choice::Both, Some(b)) = (ch, beside) {
                                    one = b;
                                }
                                if let Some(m) = one.as_object_mut() {
                                    m.remove("was");
                                }
                                text = k7_sync::k7x::apply_k7x(&text, &[one]).map_err(|e| {
                                    Error::Unparsable {
                                        path: path.clone(),
                                        message: e.to_string(),
                                    }
                                })?;
                            }
                            Some(_) => {}
                        }
                    }
                    if open {
                        out.waiting += 1;
                        continue;
                    }
                    parses(path, &text)?;
                    out.files.push((
                        path.clone(),
                        Content::Bytes {
                            bytes: text.into_bytes(),
                            mode: *mode,
                        },
                    ));
                }
                Plan::Text { segs, mode } => {
                    let mut text = String::new();
                    let mut open = false;
                    for s in segs {
                        match s {
                            Seg::Ok { text: t } => text.push_str(t),
                            Seg::Conflict { id } => match choice_of(&p.conflicts, id) {
                                None => open = true,
                                Some((c, Choice::Mine)) => {
                                    text.push_str(c.mine.as_deref().unwrap_or(""))
                                }
                                Some((c, Choice::Theirs)) => {
                                    text.push_str(c.theirs.as_deref().unwrap_or(""))
                                }
                                Some((c, _)) => {
                                    text.push_str(c.mine.as_deref().unwrap_or(""));
                                    text.push_str(c.theirs.as_deref().unwrap_or(""));
                                }
                            },
                        }
                    }
                    if open {
                        out.waiting += 1;
                        continue;
                    }
                    parses(path, &text)?;
                    out.files.push((
                        path.clone(),
                        Content::Bytes {
                            bytes: text.into_bytes(),
                            mode: *mode,
                        },
                    ));
                }
            }
        }
        Ok(out)
    }
}
