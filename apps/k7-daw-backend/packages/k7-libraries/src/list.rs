//! list.rs — what the store reads before it touches a folder: the note of what was done, the list of names, the seed.
//! What a reader gets wrong: a list that does not read is no list, and a seed entry that is no library name is dropped.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use k7_file_service::libraries::is_library_name;
use k7_file_service::paths::resolve_all;
use k7x::edit::js_trim;
use serde_json::{Value, json};

/// What `ensure` did to one library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub name: String,
    pub state: &'static str,
    pub note: String,
}

impl Note {
    pub(crate) fn new(name: &str, state: &'static str, note: impl Into<String>) -> Note {
        Note {
            name: name.into(),
            state,
            note: note.into(),
        }
    }

    pub(crate) fn failed(name: &str, e: &dyn std::fmt::Display) -> Note {
        Note::new(name, "failed", e.to_string().trim())
    }

    /// The reply's spelling.
    pub fn json(&self) -> Value {
        json!({ "name": self.name, "state": self.state, "note": self.note })
    }
}

/// Why `list` cannot be the origin's libraries, or None when it can.
pub fn settings_problem(list: &[String]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for n in list {
        if !is_library_name(n) {
            return Some(format!(
                "'{n}' cannot name a library — lowercase letters, digits and dashes, and not engine, workspace or meta"
            ));
        }
        if seen.contains(&n.as_str()) {
            return Some(format!("two libraries are called '{n}'"));
        }
        seen.push(n);
    }
    None
}

/// A `libraries.json` read as a list of names; anything unreadable is none.
pub(crate) fn parse_list(text: &str) -> Vec<String> {
    let Ok(raw) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for l in raw
        .get("libraries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(n) = l.get("name").and_then(Value::as_str)
            && is_library_name(n)
            && !out.iter().any(|o| o == n)
        {
            out.push(n.to_string());
        }
    }
    out
}

/// The library folders directly under `dir`.
pub(crate) fn library_folders(dir: &Path) -> Vec<String> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|d| d.file_type().is_ok_and(|t| t.is_dir()))
        .map(|d| d.file_name().to_string_lossy().into_owned())
        .filter(|n| is_library_name(n))
        .collect();
    out.sort();
    out
}

/// The seed's libraries by name: `name=folder,…`, or every library folder directly under one folder.
pub fn parse_seed(value: Option<&str>) -> Vec<(String, PathBuf)> {
    let mut seed: Vec<(String, PathBuf)> = Vec::new();
    let mut put = |name: String, dir: PathBuf| match seed.iter_mut().find(|(n, _)| *n == name) {
        Some(slot) => slot.1 = dir,
        None => seed.push((name, dir)),
    };
    let Some(v) = value.map(js_trim).filter(|v| !v.is_empty()) else {
        return seed;
    };
    if !v.contains('=') {
        let root = PathBuf::from(resolve_all(&[v]));
        for name in library_folders(&root) {
            put(name.clone(), root.join(&name));
        }
        return seed;
    }
    for pair in v.split(',') {
        let mut parts = pair.split('=').map(js_trim);
        if let (Some(name), Some(dir)) = (parts.next(), parts.next())
            && !name.is_empty()
            && !dir.is_empty()
            && is_library_name(name)
        {
            put(name.to_string(), PathBuf::from(resolve_all(&[dir])));
        }
    }
    seed
}

pub(crate) fn stamp() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("{ms}-{}", std::process::id())
}

pub(crate) fn rm(p: &Path) {
    let _ = fs::remove_dir_all(p);
}
