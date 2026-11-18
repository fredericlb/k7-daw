//! workspace.rs — what a workspace lists: the playlists under `_playlists/` and the reference tracks under
//! `_references/`, at any depth (1410 to 1413). A playlist is read through the k7x reader of the front.
//! What a reader gets wrong: `ok` is "the text parses"; the playlist's own shape is never judged beyond its lines.

use std::io::Read;

use super::below::{Found, files_below};
use super::{Listed, Stamp, is_doc_name, quoted, strip_doc_ext};
use crate::paths::{basename, dirname, join};
use crate::read::open_regular;
use crate::space::{Declined, Space, js_trim};
use crate::tree::Limits;

/// Where a workspace keeps its playlists and its reference tracks.
const PLAYLISTS_DIR: &str = "_playlists";
const REFERENCES_DIR: &str = "_references";
/// The extensions offered as reference tracks.
const REFERENCE_EXTS: [&str; 10] = [
    ".wav", ".flac", ".mp3", ".m4a", ".aac", ".ogg", ".opus", ".aif", ".aiff", ".webm",
];

/// `isReferenceFile`: the extension after the last dot not at the start, lower-cased, is one of the ten.
fn is_reference_file(name: &str) -> bool {
    match name.rfind('.') {
        Some(at) if at > 0 => REFERENCE_EXTS
            .iter()
            .any(|e| name[at..].eq_ignore_ascii_case(e)),
        _ => false,
    }
}

/// `referenceName`: the name before its last dot.
fn reference_name(file: &str) -> &str {
    match file.rfind('.') {
        Some(at) if at > 0 => &file[..at],
        _ => file,
    }
}

/// A playlist as the listing shows it: `None` for the title where there is none.
struct Row {
    abs: String,
    rel: String,
    name: String,
    title: Option<String>,
    count: usize,
    ok: bool,
}

/// What a playlist's text says: its trimmed title and the lines `playlistFromData` keeps.
fn parse_playlist(
    text: &str,
    bump: &k7x::Bump,
) -> Result<Option<(Option<String>, usize)>, Declined> {
    let parsed = match k7x::parse_tree_lossless(text, bump) {
        Ok(Some(tree)) => tree,
        Ok(None) => return Err(Declined::LossyText),
        Err(_) => return Ok(None),
    };
    let entry = |key: &str| {
        parsed
            .entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| &e.value)
    };
    let title = match entry("title") {
        Some(k7x::Node::Scalar {
            value: k7x::Scalar::Str(s),
            ..
        }) => Some(js_trim(s))
            .filter(|t| !t.is_empty())
            .map(str::to_string),
        _ => None,
    };
    let line_kept = |n: &k7x::Node| {
        match n {
        k7x::Node::Scalar {
            value: k7x::Scalar::Str(s),
            ..
        } => !js_trim(s).is_empty(),
        k7x::Node::Map(m) => m.entries.iter().any(|e| {
            e.key == "song"
                && matches!(&e.value, k7x::Node::Scalar { value: k7x::Scalar::Str(s), .. } if !js_trim(s).is_empty())
        }),
        _ => false,
    }
    };
    let count = match entry("items") {
        Some(k7x::Node::List { items, .. }) => items.iter().filter(|n| line_kept(n)).count(),
        _ => 0,
    };
    Ok(Some((title, count)))
}

/// A file's text as `readFileSync(path, "utf8")` gives it; `Ok(None)` where it cannot be read (an unreadable playlist is `ok: false`).
fn read_whole(
    abs: &str,
    limits: &Limits,
    admit: &dyn Fn(u64) -> bool,
) -> Result<Option<String>, Declined> {
    let Ok(file) = open_regular(abs) else {
        return Ok(None);
    };
    let len = file.metadata().map_or(0, |m| m.len());
    if len > limits.max_text {
        return Err(Declined::TooLarge);
    }
    if !admit(len) {
        return Err(Declined::Busy);
    }
    let mut bytes = Vec::new();
    // The stat may be stale: the read itself stops one byte past the limit.
    if file
        .take(limits.max_text.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(None);
    }
    if bytes.len() as u64 > limits.max_text {
        return Err(Declined::TooLarge);
    }
    Ok(Some(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }))
}

fn finish(
    mut out: Vec<u8>,
    rows: Vec<Stamp>,
    newest: u64,
    limits: &Limits,
) -> Result<Listed, Declined> {
    out.extend_from_slice(b"]}");
    if out.len() > limits.max_bytes {
        return Err(Declined::TooLarge);
    }
    Ok(Listed {
        json: String::from_utf8(out).map_err(|_| Declined::LossyText)?,
        rows,
        newest_folder_ns: newest,
        depth: 0,
        unlisted: Vec::new(),
    })
}

fn stamp_of(abs: &str) -> Stamp {
    Stamp {
        file: Some(abs.to_string()),
        folder: dirname(abs),
    }
}

/// `playlistsIn`: the playlists under `_playlists/`, sorted by path in code point order.
pub fn playlists(
    space: &Space<'_>,
    limits: &Limits,
    admit: &dyn Fn(u64) -> bool,
) -> Result<Listed, Declined> {
    let at = join(&[space.root(), PLAYLISTS_DIR]);
    let mut seen = 0;
    let below = files_below(space, &at, &is_doc_name, limits, &mut seen)?;
    let mut bump = k7x::Bump::new();
    let newest = below.newest_folder_ns;
    let mut rows: Vec<Row> = Vec::with_capacity(below.files.len());
    let mut held = 0usize;
    for found in below.files {
        let name = strip_doc_ext(&basename(&found.abs)).to_string();
        let parsed = match read_whole(&found.abs, limits, admit)? {
            Some(text) => {
                bump.reset();
                parse_playlist(&text, &bump)?
            }
            None => None,
        };
        let (title, count, ok) = match parsed {
            Some((title, count)) => (title, count, true),
            None => (None, 0, false),
        };
        held += found.rel.len() + name.len() + title.as_ref().map_or(0, String::len) + 64;
        if held > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
        rows.push(Row {
            abs: found.abs,
            rel: found.rel,
            name,
            title,
            count,
            ok,
        });
    }
    rows.sort_by(|a, b| a.rel.cmp(&b.rel));
    let mut out = b"{\"playlists\":[".to_vec();
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"path\":");
        quoted(&mut out, &r.rel);
        out.extend_from_slice(b",\"name\":");
        quoted(&mut out, &r.name);
        if let Some(t) = &r.title {
            out.extend_from_slice(b",\"title\":");
            quoted(&mut out, t);
        }
        out.extend_from_slice(format!(",\"count\":{},\"ok\":{}}}", r.count, r.ok).as_bytes());
    }
    let stamps = rows.iter().map(|r| stamp_of(&r.abs)).collect();
    finish(out, stamps, newest, limits)
}

/// `referencesIn`: the reference tracks under `_references/`, sorted by path in code point order.
pub fn references(space: &Space<'_>, limits: &Limits) -> Result<Listed, Declined> {
    let at = join(&[space.root(), REFERENCES_DIR]);
    let mut seen = 0;
    let below = files_below(space, &at, &is_reference_file, limits, &mut seen)?;
    let newest = below.newest_folder_ns;
    let mut rows: Vec<(&Found, String, u64)> = below
        .files
        .iter()
        .map(|f| {
            let bytes = std::fs::metadata(&f.abs).map_or(0, |m| m.len());
            (f, reference_name(&basename(&f.abs)).to_string(), bytes)
        })
        .collect();
    rows.sort_by(|a, b| a.0.rel.cmp(&b.0.rel));
    let mut out = b"{\"references\":[".to_vec();
    for (i, (f, name, bytes)) in rows.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"path\":");
        quoted(&mut out, &f.rel);
        out.extend_from_slice(b",\"name\":");
        quoted(&mut out, name);
        out.extend_from_slice(format!(",\"bytes\":{bytes}}}").as_bytes());
        if out.len() > limits.max_bytes {
            return Err(Declined::TooLarge);
        }
    }
    let stamps = rows.iter().map(|r| stamp_of(&r.0.abs)).collect();
    finish(out, stamps, newest, limits)
}
