//! chains/cold_run.rs — `resolveColdRun`: which model a cut runs and what it writes, with the TypeScript's statuses and sentences.
//! What a reader gets wrong: a live step is counted as an engine plugin, as nothing native knows the plugin cache (1p-D14).

use super::cold::{ColdStep, offline_steps, split_running};
use super::{ChainSettings, Chains};
use crate::libraries::is_library_name;
use crate::space::Declined;

pub(crate) const DEFAULT_LIBRARY: &str = "k7-core";
const INSTRUMENTAL: &str = "instrumental";

/// What a cut resolves to: the model and the outputs, or a refusal with its status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColdRun {
    Ok { model: String, outputs: Vec<String> },
    Refused { status: u16, error: String },
}

fn refused(status: u16, error: impl Into<String>) -> Result<ColdRun, Declined> {
    Ok(ColdRun::Refused {
        status,
        error: error.into(),
    })
}

/// JavaScript's `trim`: its WhiteSpace and LineTerminator, which is not Rust's.
pub(crate) fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
    })
}

/// `libraryRefParts`: `@<library>/<rest>` with a library name the account may have, else none.
pub(crate) fn library_ref_parts(r: &str) -> Option<(&str, &str)> {
    let body = r.strip_prefix('@')?;
    let (lib, rest) = body.split_once('/')?;
    let line_end = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    (!lib.is_empty()
        && !rest.is_empty()
        && !rest.contains(line_end)
        && (is_library_name(lib) || lib == "account"))
        .then_some((lib, rest))
}

/// `findLibraryChainBy`: by path, then by last segment, the default library first and the rest in ref order (1p-D25).
pub fn find_by(c: &Chains<'_>, library: Option<&str>, want: &str) -> Option<usize> {
    let mut order: Vec<usize> = (0..c.dirs.len())
        .filter(|&i| library.is_none_or(|l| c.dirs[i].library == l))
        .collect();
    order.sort_by_key(|&i| c.dirs[i].library != DEFAULT_LIBRARY);
    order
        .iter()
        .copied()
        .find(|&i| c.dirs[i].path == want)
        .or_else(|| {
            order
                .iter()
                .copied()
                .find(|&i| c.dirs[i].path.rsplit('/').next() == Some(want))
        })
}

/// What a cut of `want` runs through `chain` or the account's setting; a chain with a live step is refused in the TypeScript's words.
pub fn resolve_cold_run(
    c: &Chains<'_>,
    prefs: &ChainSettings,
    want: &str,
    chain: &str,
) -> Result<ColdRun, Declined> {
    let lowered = js_trim(want).to_lowercase();
    let w = if lowered.is_empty() {
        "stems"
    } else {
        lowered.as_str()
    };
    if w != "stems" && w != INSTRUMENTAL {
        return refused(400, format!("want must be 'stems' or '{INSTRUMENTAL}'"));
    }
    let mix = w == INSTRUMENTAL;
    let named = js_trim(chain);
    let setting = if mix {
        &prefs.instrumental
    } else {
        &prefs.stems
    };
    let r = if named.is_empty() {
        setting.as_deref().unwrap_or("")
    } else {
        named
    };
    if r.is_empty() {
        let (a, b) = if mix {
            ("makes no instrumental", "system instrumental")
        } else {
            ("separates nothing", "system stems")
        };
        return refused(
            409,
            format!(
                "no chain named, and this account {a} yet — name one, or set one once with `{b} <chaîne>`"
            ),
        );
    }
    if !c.cloned() {
        return refused(
            409,
            "this machine has cloned none of the account's libraries yet — add one on k7's home page, or open a project once",
        );
    }
    let trimmed = js_trim(r);
    let parts = library_ref_parts(trimmed);
    let wanted = parts.map_or(trimmed, |(_, rest)| rest).trim_matches('/');
    let Some(at) = find_by(c, parts.map(|(l, _)| l), wanted) else {
        let known: Vec<&str> = c.dirs.iter().map(|d| d.ref_.as_str()).collect();
        let none = if known.is_empty() { "none" } else { "" };
        let tail = if c.dirs.is_empty() {
            " (this machine has cloned no library holding chains)"
        } else {
            ""
        };
        return refused(
            400,
            format!(
                "no chain '{trimmed}' — known here: {}{none}{tail}",
                known.join(", ")
            ),
        );
    };
    let found = &c.dirs[at];
    if let Some(m) = c.unreadable(at) {
        return refused(400, m);
    }
    let Some((steps, lost)) = offline_steps(c, at)? else {
        let (missing, live, cold) = split_running(c, at)?;
        if !missing.is_empty() {
            return refused(
                400,
                format!(
                    "chain '{}' has steps this machine cannot load: {}",
                    found.ref_,
                    missing.join(", ")
                ),
            );
        }
        if !cold.is_empty() {
            return refused(
                400,
                format!(
                    "chain '{}' both separates ({}) and processes ({live} module(s)) — a bounce cannot say which of the stems the next step treats. Put the separation in a chain of its own.",
                    found.ref_,
                    cold.join(", ")
                ),
            );
        }
        return refused(
            400,
            format!("'{trimmed}' does not separate — it puts the signal through {live} module(s)"),
        );
    };
    let mut missing = lost;
    let mut cold: Vec<ColdStep> = Vec::new();
    for s in steps {
        match s.cold {
            Ok(step) => cold.push(step),
            Err(e) => missing.push(e),
        }
    }
    if !missing.is_empty() {
        return refused(
            400,
            format!(
                "chain '{}' has steps this machine cannot load: {}",
                found.ref_,
                missing.join(", ")
            ),
        );
    }
    let Some(step) = cold.first() else {
        return refused(
            400,
            format!("chain '{}' puts the signal through no module", found.ref_),
        );
    };
    if mix && !step.outputs.iter().any(|o| o == INSTRUMENTAL) {
        return refused(
            400,
            format!(
                "'{r}' writes {} and no instrumental",
                step.outputs.join(", ")
            ),
        );
    }
    Ok(ColdRun::Ok {
        model: step.model.clone(),
        outputs: step.outputs.clone(),
    })
}
