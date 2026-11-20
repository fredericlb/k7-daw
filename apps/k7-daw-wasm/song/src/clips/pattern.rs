// clips/pattern.rs — a strudel lane compiled for the engine: the plugin heading each half, with the file's text and its share.
// A reader would get wrong that the file is read through the Source and capped: the TypeScript read any path the schema admits, unconfined.

use super::doc::{PatternPlugin, Plays};
use super::{error_in, warning_in};
use crate::files::Finding;
use crate::roots::{Roots, is_absolute, spell};
use crate::source::path::PathRule;
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError, Span};
use indexmap::IndexMap;

/// Must match PatternPlugin::xmlTypeName in the engine.
pub const PATTERN_PLUGIN_TYPE: &str = "studio_pattern";

/// A pattern file larger than this is not sent to the engine (decision 164).
pub const MAX_PATTERN: u64 = 1024 * 1024;

/// Pattern lanes one file serves; each carries the file's text and every claimed name, so the cost is the square (decision 178).
pub const MAX_FILE_LANES: usize = 32;

/// Pattern lanes one project may have, whatever their files.
pub const MAX_LANES: usize = 128;

/// How one lane shares its pattern file: every label the file's lanes claim, and whether it takes the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternShare {
    pub claimed: Vec<String>,
    pub catch_all: bool,
}

/// One engine track of a lane: its id and the port it was keyed on ("audio", "midi"), none when it takes what is left.
#[derive(Clone, Copy, Debug)]
pub struct Half<'a> {
    pub id: &'a str,
    pub port: Option<&'a str>,
}

/// The path a pattern file is keyed by: normalized under the project, or its own spelling when it will not normalize.
fn file_key(project: &str, strudel: &str, rule: PathRule) -> String {
    let path = if is_absolute(strudel, rule) {
        spell(strudel, rule)
    } else {
        spell(project, rule).and_then(|p| spell(&join(&p, strudel), rule))
    };
    path.unwrap_or_else(|_| strudel.to_owned())
}

/// Per pattern lane, its share of its file; the file's FIRST lane by name takes what no lane claims.
/// `lanes` is (lane, strudel path) of the lanes that play a pattern. A lane past `MAX_FILE_LANES` of its file or
/// `MAX_LANES` of the project has no share and plays nothing, one ERROR per file naming it (decision 178).
pub fn pattern_shares(
    project: &str,
    lanes: &[(&str, &str)],
    rule: PathRule,
) -> (IndexMap<String, PatternShare>, Vec<Finding>) {
    let mut sorted: Vec<&(&str, &str)> = lanes.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut by_file: IndexMap<String, Vec<&str>> = IndexMap::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut kept = 0;
    for (name, strudel) in sorted {
        let key = file_key(project, strudel, rule);
        let names = by_file.entry(key.clone()).or_default();
        if names.len() >= MAX_FILE_LANES || kept >= MAX_LANES {
            if !dropped.contains(&key) {
                dropped.push(key);
                findings.push(error_in(
                    strudel,
                    format!("too many lanes play patterns here ({MAX_FILE_LANES} to a file, {MAX_LANES} in all) — the rest play nothing"),
                ));
            }
            continue;
        }
        names.push(name);
        kept += 1;
    }
    let mut out = IndexMap::new();
    for names in by_file.values() {
        let claimed: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
        for (i, name) in names.iter().enumerate() {
            out.insert(
                (*name).to_owned(),
                PatternShare {
                    claimed: claimed.clone(),
                    catch_all: i == 0,
                },
            );
        }
    }
    (out, findings)
}

/// Where a sound name is looked for as a sample, in order: the project's own, then the folders the host names.
pub fn sample_banks(roots: &Roots) -> Vec<String> {
    let own =
        spell(&roots.project, roots.paths).map_or_else(|_| join(&roots.project, "samples"), |p| join(&p, "samples"));
    std::iter::once(own).chain(roots.sample_banks.iter().cloned()).collect()
}

fn refused(strudel: &str, why: &str) -> (String, Option<Finding>) {
    (
        String::new(),
        Some(warning_in(
            strudel,
            format!("this pattern file is {why}, so the lane plays nothing"),
        )),
    )
}

/// A pattern file's text, read once through the Source; "" and a warning named by `strudel` when it may not be sent.
pub fn read_pattern(
    source: &dyn Source,
    project: &str,
    strudel: &str,
    rule: PathRule,
) -> Result<(String, Option<Finding>), SourceError> {
    let path = if is_absolute(strudel, rule) {
        spell(strudel, rule)
    } else {
        spell(project, rule).and_then(|p| spell(&join(&p, strudel), rule))
    };
    // A path no spelling reaches (a NUL) is outside by any reading.
    let Ok(path) = path else {
        return Ok(refused(strudel, "outside the project's folders"));
    };
    match source.stat(&path) {
        Ok(st) if st.kind != Kind::File => return Ok((String::new(), None)),
        Ok(st) if st.len > MAX_PATTERN => return Ok(refused(strudel, "too large to send")),
        Ok(_) => {}
        Err(SourceError::NotFound | SourceError::Io(_)) => return Ok((String::new(), None)),
        Err(SourceError::Outside) => return Ok(refused(strudel, "outside the project's folders")),
        Err(e) => return Err(e),
    }
    let bytes = match source.read(&path, Span::All) {
        Ok(b) => b,
        Err(SourceError::NotFound | SourceError::Io(_)) => return Ok((String::new(), None)),
        Err(SourceError::Outside) => return Ok(refused(strudel, "outside the project's folders")),
        Err(e) => return Err(e),
    };
    if bytes.len() as u64 > MAX_PATTERN {
        return Ok(refused(strudel, "too large to send"));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok((text, None)),
        Err(_) => Ok(refused(strudel, "not text")),
    }
}

/// The pattern plugin heading each of the lane's halves, by half id: one playing both on a bare-wired lane,
/// else the notes on `midi` and the samples on `audio`.
pub fn pattern_plugins(
    lane: &str,
    halves: &[Half<'_>],
    text: &str,
    share: &PatternShare,
    banks: &[String],
) -> IndexMap<String, PatternPlugin> {
    let find = |port: &str| {
        halves
            .iter()
            .position(|h| h.port == Some(port))
            .or_else(|| halves.iter().position(|h| h.port.is_none()))
    };
    let (notes, audio) = (find("midi"), find("audio"));
    // Halves are compared by POSITION: the TypeScript compared the objects.
    let mut plays: Vec<(usize, Plays)> = Vec::new();
    match (notes, audio) {
        (Some(n), Some(a)) if n == a => plays.push((n, Plays::Both)),
        (n, a) => {
            plays.extend(n.map(|n| (n, Plays::Notes)));
            plays.extend(a.map(|a| (a, Plays::Samples)));
        }
    }
    let only = plays.len() == 1;
    let mut out = IndexMap::new();
    for (i, what) in plays {
        let id = halves[i].id;
        out.insert(
            id.to_owned(),
            PatternPlugin {
                id: format!("{id}:pattern"),
                plugin_type: PATTERN_PLUGIN_TYPE.to_owned(),
                name: "Pattern".to_owned(),
                path: String::new(),
                preset_save: String::new(),
                role: String::new(),
                enabled: true,
                params: IndexMap::new(),
                pattern_source: text.to_owned(),
                pattern_label: lane.to_owned(),
                pattern_catch_all: share.catch_all,
                pattern_claimed: share.claimed.clone(),
                pattern_plays: what,
                pattern_only: only,
                pattern_banks: banks.to_vec(),
            },
        );
    }
    out
}
