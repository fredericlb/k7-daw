//! names.rs — `/export`'s file name, its `timeline` and `format` asks and its `Content-Disposition` (2520, 2521).
//! What a reader gets wrong: a repeated or structured `timeline` is refused, never read as Main (the TypeScript departs from its first form).

use super::qs::{Q, query_value};
use crate::paths::basename;

/// What `?timeline` asked for, as Express' query parser reads it.
#[derive(Debug, PartialEq, Eq)]
pub enum TimelineAsk {
    Absent,
    Name(String),
    /// Repeated, `timeline[]=` or `timeline[k]=`: the JSON of what the parser made of it.
    Refused(String),
}

/// `req.query.timeline` of a query string.
pub fn timeline_ask(query: &str) -> TimelineAsk {
    match query_value(query, "timeline") {
        None => TimelineAsk::Absent,
        Some(Q::Str(s)) => TimelineAsk::Name(s),
        Some(other) => TimelineAsk::Refused(other.json()),
    }
}

/// The timeline `/export` is asked for, or the 400's sentence; an empty name is Main.
pub fn check_timeline(ask: &TimelineAsk) -> Result<Option<&str>, String> {
    match ask {
        TimelineAsk::Absent => Ok(None),
        TimelineAsk::Name(n) if n.is_empty() => Ok(None),
        TimelineAsk::Name(n)
            if n.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) =>
        {
            Ok(Some(n))
        }
        TimelineAsk::Name(n) => Err(format!("'{n}' names no timeline")),
        TimelineAsk::Refused(json) => Err(format!("'{json}' names no timeline")),
    }
}

/// The sentence for a well-formed timeline the project does not hold.
pub fn no_such_timeline(id: &str) -> String {
    format!("no timeline \"{id}\" to export")
}

/// Whether `?format` is MP3: `String(req.query.format ?? "wav") === "mp3"`, an array reading as its elements joined by commas.
pub fn wants_mp3(query: &str) -> bool {
    query_value(query, "format").is_some_and(|q| q.string() == "mp3")
}

/// The mixdown's name without its extension: the document's, else the folder's, else `mixdown`; a timeline's id is appended.
pub fn mix_name(doc_name: &str, dir: &str, timeline: Option<&str>) -> String {
    let folder = basename(dir);
    let base = [doc_name, folder.as_str(), "mixdown"]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("mixdown");
    let name = match timeline {
        Some(t) => format!("{base}-{t}"),
        None => base.to_owned(),
    };
    let mut out = String::with_capacity(name.len());
    let mut in_run = false;
    for c in name.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            if !in_run {
                out.push('_');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// An attachment header any name survives: printable ASCII in `filename`, the whole name percent-encoded in `filename*` (2521).
pub fn content_disposition(filename: &str) -> String {
    let plain: String = filename
        .chars()
        .map(|c| {
            if (' '..='~').contains(&c) && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::with_capacity(filename.len());
    for b in filename.bytes() {
        if b.is_ascii_alphanumeric() || b"!#$&+.^_`|~-".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!("attachment; filename=\"{plain}\"; filename*=UTF-8''{encoded}")
}
