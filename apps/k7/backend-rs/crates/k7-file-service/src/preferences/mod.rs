//! preferences — the account's `preferences.k7x` found, upgraded from a legacy YAML once, and changed (`preferences.ts`, `workspace.ts` `preferences()`).
//! What a reader gets wrong: a file that does not read is never saved over; every change names the problem instead.

mod upgrade;
mod yaml;

use song_edit::preferences::{
    LEGACY_PREFERENCES_FILE, PREFERENCES_FILE, Preferences, format_audio_device, format_channels,
    normalize_preferences, parse_strict, stringify_preferences,
};

use self::upgrade::{Read, Spec, Upgraded, read_text, upgrade_legacy};
use crate::atomic::write_atomic;
use crate::libraries::Home;
use crate::paths::{dirname, join};

pub use self::upgrade::BACKUP_SUFFIX;

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

fn legacy_values(text: &str) -> Option<Preferences> {
    yaml::parse(text).map(|v| normalize_preferences(&v))
}

fn mode_of(file: &str) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(file)
            .ok()
            .map(|m| m.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        None
    }
}

fn is_link(file: &str) -> bool {
    std::fs::symlink_metadata(file).is_ok_and(|m| m.is_symlink())
}

/// Reads the files as they are, upgrading a legacy YAML once; a change never goes on from a YAML under another's upgrade.
pub fn find(home: &Home, for_write: bool) -> Found {
    find_waiting(home, for_write, for_write)
}

/// `find`, with the wait for a held upgrade lock named apart from the intent: a change that asks no wait is the intent alone.
pub fn find_waiting(home: &Home, for_write: bool, wait: bool) -> Found {
    let (path, legacy) = (
        join(&[&home.studio, PREFERENCES_FILE]),
        join(&[&home.studio, LEGACY_PREFERENCES_FILE]),
    );
    for _ in 0..6 {
        let k = read_text(&path);
        if let Read::Error(e) = &k {
            return unreadable(&path, &format!("cannot be read ({e})"));
        }
        // An empty k7x is a write that never finished: it reads as a file that is not there.
        let blank = matches!(&k, Read::Text(t) if t.is_empty());
        if let Read::Text(t) = &k
            && !blank
        {
            return parse_strict(t).map_or_else(
                || unreadable(&path, "does not read"),
                |prefs| Found {
                    prefs,
                    problem: None,
                },
            );
        }
        if matches!(k, Read::Absent) && is_link(&path) {
            return unreadable(&path, "is a link to nothing");
        }
        let y = match read_text(&legacy) {
            Read::Error(e) => return unreadable(&legacy, &format!("cannot be read ({e})")),
            Read::Absent => {
                if blank {
                    return unreadable(&path, "is empty");
                }
                if matches!(read_text(&path), Read::Absent) {
                    return Found {
                        prefs: Preferences::default(),
                        problem: None,
                    };
                }
                continue;
            }
            Read::Text(y) => y,
        };
        if legacy_values(&y).is_none() {
            return unreadable(&legacy, "does not read");
        }
        let spec = Spec {
            legacy: &legacy,
            target: &path,
            from_legacy: &legacy_values,
            print: &|p: &Preferences| stringify_preferences(p).ok(),
            read_back: &parse_strict,
            equal: &|a: &Preferences, b: &Preferences| a == b,
            mode: mode_of(&legacy),
            wait,
        };
        match upgrade_legacy(&spec) {
            Upgraded::Retry => continue,
            Upgraded::Values(prefs, busy) => {
                if busy && for_write {
                    let again = read_text(&path);
                    if matches!(&again, Read::Error(_))
                        || matches!(&again, Read::Text(t) if !t.is_empty())
                    {
                        continue;
                    }
                    return Found {
                        prefs,
                        problem: Some(format!(
                            "{path} is being upgraded by another process; nothing was saved, try again"
                        )),
                    };
                }
                return Found {
                    prefs,
                    problem: None,
                };
            }
        }
    }
    unreadable(&path, "kept changing while it was read")
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
