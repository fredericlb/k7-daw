//! plan.rs — the set judged before anything is written: each path through the gates, each write through its guards, each
//! written `.k7x` parsed. What a reader gets wrong: the disk is asked only whether a source is a file; whether a file moved is `fresh`'s.

use std::collections::HashSet;

use super::{MAX_OPS, MAX_TEXT, Op, Refusal};
use crate::confine::Disk;
use crate::entries::guards::{assert_library_writable, refuse_yaml};
use crate::filter::{is_binary, skip};
use crate::paths::{basename, join, resolve_all};
use crate::space::{Declined, PathError, Space};

/// The folder of a job's staged outputs, below the account's home (3f's routes write there).
pub const JOB_OUT: &str = ".job-out";

/// One op with its paths resolved: what `fresh` compares and `commit` lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Text {
        rel: String,
        abs: String,
        text: String,
        was: Option<String>,
    },
    Remove {
        rel: String,
        abs: String,
        was: Option<String>,
    },
    /// A check only: nothing is written, and `target` is not a written name.
    Read {
        rel: String,
        abs: String,
        was: Option<String>,
    },
    Copy {
        rel: String,
        abs: String,
        from: String,
        replace: bool,
    },
    Move {
        rel: String,
        abs: String,
        from: String,
        replace: bool,
    },
    Adopt {
        rel: String,
        abs: String,
        from: String,
        replace: bool,
    },
}

impl Act {
    /// The file the act writes or removes: as the op spelled it, and resolved.
    pub fn target(&self) -> (&str, &str) {
        match self {
            Act::Text { rel, abs, .. }
            | Act::Remove { rel, abs, .. }
            | Act::Read { rel, abs, .. }
            | Act::Copy { rel, abs, .. }
            | Act::Move { rel, abs, .. }
            | Act::Adopt { rel, abs, .. } => (rel, abs),
        }
    }
}

/// A set that passed every check that needs no disk state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub acts: Vec<Act>,
}

fn bad(s: impl Into<String>) -> Refusal {
    Refusal::Bad(s.into())
}

fn segments(rel: &str) -> impl Iterator<Item = &str> {
    rel.split(['\\', '/'])
        .filter(|s| !s.is_empty() && *s != ".")
}

/// `rel` through the gates: a name that hides, then `safePath`'s two; the absolute path, or why not.
fn gate(space: &Space<'_>, rel: &str) -> Result<String, Refusal> {
    if segments(rel).next().is_none() {
        return Err(bad(format!("'{rel}' names no file")));
    }
    if segments(rel).any(|s| s != ".." && skip(s)) {
        return Err(Refusal::Forbidden(format!(
            "'{rel}' is hidden state, which a change set never touches"
        )));
    }
    space.safe_path(rel, &Disk).map_err(|e| match e {
        PathError::Declined(Declined::LongPath) => bad(format!("'{rel}' is too long")),
        PathError::Declined(d) => Refusal::Declined(format!("declined: {}", d.as_str())),
        e => Refusal::Forbidden(e.to_string()),
    })
}

/// A path written: past the gates, writable (a library is the admins'), not a YAML name.
fn gate_write(space: &Space<'_>, rel: &str) -> Result<String, Refusal> {
    let abs = gate(space, rel)?;
    assert_library_writable(space, &abs, "write").map_err(|e| Refusal::Forbidden(e.to_string()))?;
    refuse_yaml(rel).map_err(|e| bad(e.to_string()))?;
    Ok(abs)
}

/// A source that must be a regular file now: a link is not followed.
fn source_file(abs: &str, shown: &str) -> Result<(), Refusal> {
    match std::fs::symlink_metadata(abs) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(m) if m.is_dir() => Err(bad(format!("'{shown}' is a folder, not a file"))),
        Ok(_) => Err(bad(format!("'{shown}' is not a regular file"))),
        Err(_) => Err(bad(format!("'{shown}': no such file"))),
    }
}

/// A folder on the way to a staged output that is a link is refused: the output is read through it, so it is not the job's.
fn no_link(dir: &str, shown: &str) -> Result<(), Refusal> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if m.is_symlink() => Err(bad(format!("'{shown}' is not a staged output"))),
        _ => Ok(()),
    }
}

pub(crate) fn one_segment(s: &str) -> bool {
    !s.is_empty() && s.len() <= 255 && !s.starts_with('.') && !s.contains(['/', '\\', '\0'])
}

pub(crate) fn job_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A `.k7x` the set writes must parse (3-B7): the Rust reader's own message and position.
fn parse_k7x(rel: &str, text: &str) -> Result<(), Refusal> {
    if !rel.to_ascii_lowercase().ends_with(".k7x") {
        return Ok(());
    }
    let bump = k7x::Bump::new();
    k7x::parse_tree(text, &bump)
        .map(|_| ())
        .map_err(|e| Refusal::Unparsable {
            file: rel.to_string(),
            error: format!("{rel}: {e}"),
        })
}

/// The set's acts: every path resolved and gated, every written text parsed, and nothing of the disk's state read but "a source is a file".
pub fn plan(space: &Space<'_>, ops: &[Op]) -> Result<Planned, Refusal> {
    let weight = ops.iter().fold(0usize, |n, o| match o {
        Op::Text { text, .. } => n.saturating_add(text.len()),
        _ => n,
    });
    if ops.len() > MAX_OPS || weight > MAX_TEXT {
        return Err(Refusal::TooMany("the change set is past its limits".into()));
    }
    let studio = &space.home().studio;
    let mut acts = Vec::with_capacity(ops.len());
    // Gates and permission first, for every op, so a forbidden set never says "stale" or "unparsable".
    for o in ops {
        acts.push(match o {
            Op::Text { file, text, was } => {
                let abs = gate_write(space, file)?;
                if is_binary(&basename(&abs)) {
                    return Err(bad("cannot edit a binary file"));
                }
                Act::Text {
                    rel: file.clone(),
                    abs,
                    text: text.clone(),
                    was: was.clone(),
                }
            }
            Op::Remove { file, was } => Act::Remove {
                rel: file.clone(),
                abs: gate_write(space, file)?,
                was: was.clone(),
            },
            Op::Read { file, was } => Act::Read {
                rel: file.clone(),
                abs: gate(space, file)?,
                was: was.clone(),
            },
            Op::Copy {
                from,
                file,
                replace,
            } => {
                let src = gate(space, from)?;
                let abs = gate_write(space, file)?;
                source_file(&src, from)?;
                Act::Copy {
                    rel: file.clone(),
                    abs,
                    from: src,
                    replace: *replace,
                }
            }
            Op::Move {
                from,
                file,
                replace,
            } => {
                let src = gate_write(space, from)?;
                let abs = gate_write(space, file)?;
                source_file(&src, from)?;
                Act::Move {
                    rel: file.clone(),
                    abs,
                    from: src,
                    replace: *replace,
                }
            }
            Op::Adopt {
                job,
                out,
                file,
                replace,
            } => {
                if !job_id(job) || !one_segment(out) {
                    return Err(bad(format!("'{job}/{out}' names no staged output")));
                }
                let abs = gate_write(space, file)?;
                let src = join(&[studio, JOB_OUT, job, out]);
                for dir in [join(&[studio, JOB_OUT]), join(&[studio, JOB_OUT, job])] {
                    no_link(&dir, &format!("{job}/{out}"))?;
                }
                source_file(&src, &format!("{job}/{out}"))?;
                Act::Adopt {
                    rel: file.clone(),
                    abs,
                    from: src,
                    replace: *replace,
                }
            }
        });
    }
    // One written file once: the order of two ops on a file would be the set's meaning.
    let mut written: HashSet<String> = HashSet::new();
    for a in acts.iter().filter(|a| !matches!(a, Act::Read { .. })) {
        let mut names = vec![resolve_all(&[a.target().1])];
        if let Act::Move { from, .. } = a {
            names.push(resolve_all(&[from]));
        }
        for n in names {
            if !written.insert(n) {
                return Err(bad(format!("two ops name '{}'", a.target().0)));
            }
        }
    }
    for a in &acts {
        if let Act::Copy { from, .. } = a
            && written.contains(&resolve_all(&[from]))
        {
            return Err(bad(format!(
                "'{}' is copied from a file the same set writes",
                a.target().0
            )));
        }
    }
    for a in &acts {
        if let Act::Text { rel, text, .. } = a {
            parse_k7x(rel, text)?;
        }
    }
    Ok(Planned { acts })
}
