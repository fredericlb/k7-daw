//! fresh.rs — the set against the disk now (3-B2): the project's token, then each written file's `was`. What a reader gets wrong:
//! a text op whose text is what the file already holds is never stale, as `PUT /file` lets an identical save through.

use super::Refusal;
use super::plan::{Act, Planned};
use crate::filter::is_binary;
use crate::paths::basename;
use crate::text_hash::text_hash;

/// The text of a file now: `None` when absent, an error where it is a folder or is not text.
fn text_now(abs: &str, shown: &str) -> Result<Option<String>, Refusal> {
    match std::fs::read(abs) {
        Ok(b) => String::from_utf8(b)
            .map(Some)
            .map_err(|_| Refusal::Bad(format!("'{shown}' is not UTF-8 text"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) if std::fs::metadata(abs).is_ok_and(|m| m.is_dir()) => {
            Err(Refusal::Bad(format!("'{shown}' is a folder, not a file")))
        }
        Err(e) => Err(Refusal::Bad(format!(
            "'{shown}' could not be read ({})",
            crate::filter::code_of(&e)
        ))),
    }
}

fn exists(abs: &str) -> bool {
    std::fs::symlink_metadata(abs).is_ok()
}

/// The files whose `was` or destination moved, and whether `base` is not the token now.
pub fn fresh(planned: &Planned, base: &str, token_now: &str) -> Result<(), Refusal> {
    let mut files = Vec::new();
    for a in &planned.acts {
        match a {
            Act::Text {
                rel,
                abs,
                text,
                was,
            } => {
                let now = text_now(abs, rel)?;
                let same = now.as_deref() == Some(text.as_str());
                let held = match (&now, was) {
                    (None, None) => true,
                    (Some(n), Some(h)) => text_hash(n) == *h,
                    _ => false,
                };
                if !(same || held) {
                    files.push(rel.clone());
                }
            }
            Act::Remove { rel, abs, was } | Act::Read { rel, abs, was } => {
                if is_binary(&basename(abs)) {
                    continue;
                }
                let now = text_now(abs, rel)?;
                let held = match (&now, was) {
                    (None, None) => true,
                    (Some(n), Some(h)) => text_hash(n) == *h,
                    _ => false,
                };
                if !held {
                    files.push(rel.clone());
                }
            }
            Act::Copy {
                rel, abs, replace, ..
            }
            | Act::Move {
                rel, abs, replace, ..
            }
            | Act::Adopt {
                rel, abs, replace, ..
            } => {
                if !replace && exists(abs) {
                    files.push(rel.clone());
                }
            }
        }
    }
    let project = base != token_now;
    if files.is_empty() && !project {
        return Ok(());
    }
    Err(Refusal::Stale { files, project })
}
