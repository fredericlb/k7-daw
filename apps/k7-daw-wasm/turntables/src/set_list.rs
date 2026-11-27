// set_list.rs — the PLAYLIST the surface names, resolved against the workspace's audio files.
// A reader would get wrong that two failures are kept apart on purpose: a set list that will not parse, and a line naming no file the workspace holds.

use serde::Serialize;
use song_edit::js_text::js_trim;
use song_edit::playlist::parse_playlist;
use song_edit::records::{record_folder, record_name, resolve_record};
use std::collections::HashSet;

/// One line of the set list: what the playlist named, and whether the workspace holds it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetEntry {
    /// As written in the playlist, its slashes trimmed.
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub name_ref: String,
    /// The record's workspace path, else the ref: what names a row and its cues.
    pub id: String,
    /// What to show: the file's name without its extension.
    pub name: String,
    /// The folder holding it, shown as the record's artist; null at the root.
    pub artist: Option<String>,
    /// The record's workspace path when the file is there, else null: nothing to play.
    pub file: Option<String>,
    /// Whether its four stems are beside it.
    pub stems: bool,
}

/// What the playlist said that could not be read; the page turns each into its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SetIssue {
    NotFound,
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetList {
    pub entries: Vec<SetEntry>,
    pub issues: Vec<SetIssue>,
    /// The surface names NO playlist: distinct from an empty one and from a missing one.
    pub absent: bool,
}

fn failed(issue: SetIssue) -> SetList {
    SetList {
        entries: Vec::new(),
        issues: vec![issue],
        absent: false,
    }
}

/// Resolve a playlist against the workspace's files: every line kept, duplicates dropped.
pub fn resolve_set(text: Option<&str>, files: Option<&[String]>) -> SetList {
    let Some(text) = text else {
        return failed(SetIssue::NotFound);
    };
    let Some(parsed) = parse_playlist(text) else {
        return failed(SetIssue::Unreadable);
    };
    let held: Option<HashSet<&str>> = files.map(|f| f.iter().map(String::as_str).collect());
    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();
    for item in &parsed.items {
        let r = resolve_record(&item.song, held.as_ref());
        let name_ref = js_trim(&item.song).trim_matches('/').to_owned();
        let id = r.path.clone().unwrap_or_else(|| name_ref.clone());
        if !seen.insert(id.clone()) {
            continue;
        }
        entries.push(SetEntry {
            name_ref,
            name: record_name(&id).to_owned(),
            artist: r.path.as_deref().and_then(record_folder).map(str::to_owned),
            file: if r.present { r.path.clone() } else { None },
            stems: r.stems,
            id,
        });
    }
    SetList {
        entries,
        issues: Vec::new(),
        absent: false,
    }
}
