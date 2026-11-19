//! preferences — the account's `preferences.k7x` found and changed (`preferences.ts`, `workspace.ts` `preferences()`).
//! What a reader gets wrong: a file that does not read is never saved over; every change names the problem instead.

mod file;

use song_edit::preferences::{
    PREFERENCES_FILE, Preferences, format_audio_device, format_channels, parse_strict,
    stringify_preferences,
};

use self::file::{Read, read_text};
use crate::atomic::write_atomic;
use crate::libraries::Home;
use crate::paths::{dirname, join};

/// The preferences as found, and the reason when a file is there that does not read.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub prefs: Preferences,
    pub problem: Option<String>,
}

fn unreadable(file: &str, why: &str) -> Found {
    Found {
        prefs: Preferences::default(),
        problem: Some(format!(
            "{file} {why}; fix it by hand or move it away, nothing was saved"
        )),
    }
}

fn is_link(file: &str) -> bool {
    std::fs::symlink_metadata(file).is_ok_and(|m| m.is_symlink())
}

/// Reads `preferences.k7x` as it is; a file that does not read is the problem, never the defaults.
pub fn find(home: &Home, _for_write: bool) -> Found {
    let path = join(&[&home.studio, PREFERENCES_FILE]);
    match read_text(&path) {
        Read::Error(e) => unreadable(&path, &format!("cannot be read ({e})")),
        // An empty k7x is a write that never finished.
        Read::Text(t) if t.is_empty() => unreadable(&path, "is empty"),
        Read::Text(t) => parse_strict(&t).map_or_else(
            || unreadable(&path, "does not read"),
            |prefs| Found {
                prefs,
                problem: None,
            },
        ),
        Read::Absent if is_link(&path) => unreadable(&path, "is a link to nothing"),
        Read::Absent => Found {
            prefs: Preferences::default(),
            problem: None,
        },
    }
}

/// Where a save lands: through a link, so a dotfiles setup keeps its link; a link to nothing is refused.
fn write_target(file: &str) -> Result<String, String> {
    if !is_link(file) {
        return Ok(file.to_owned());
    }
    std::fs::canonicalize(file)
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned))
        .ok_or_else(|| format!("{file} is a link to nothing, nothing was saved"))
}

fn save(home: &Home, p: &Preferences) -> Result<(), String> {
    let path = join(&[&home.studio, PREFERENCES_FILE]);
    std::fs::create_dir_all(dirname(&path)).map_err(|e| e.to_string())?;
    let text = stringify_preferences(p).map_err(|e| e.to_string())?;
    write_atomic(&write_target(&path)?, text.as_bytes()).map_err(|e| e.to_string())
}

/// Load, change, save — and never save over a file that did not read, which would replace it with the defaults.
///
/// # Errors
/// The problem with the file, or the write's.
pub fn update(home: &Home, change: impl FnOnce(&mut Preferences)) -> Result<Preferences, String> {
    let mut found = find(home, true);
    if let Some(problem) = found.problem {
        return Err(problem);
    }
    change(&mut found.prefs);
    save(home, &found.prefs)?;
    Ok(found.prefs)
}

/// `workspace.ts` `preferences()`: the view `GET /preferences` answers.
pub fn view(home: &Home) -> serde_json::Value {
    let p = find(home, false).prefs;
    let (input, output) = format_channels(&p);
    serde_json::json!({
        "audioDevice": p.audio_device.as_ref().map(|d| format_audio_device(Some(d))),
        "channels": { "in": input, "out": output },
        "vim": p.editor_vim,
        "stemsChain": p.stems_chain,
        "instrumentalChain": p.instrumental_chain,
    })
}
