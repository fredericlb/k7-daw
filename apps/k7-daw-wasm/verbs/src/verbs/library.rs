// library.rs — `library versions` and `library upgrade`: which version of each library the WORKSPACE plays, after verbs/library.ts.
// A reader would get wrong where the pin is: `_library-pins.k7x` at the workspace root, outside every project's change set.

use super::shared::{Done, join, split_words};
use crate::host::Host;
use k7x::edit::{PathSeg, Value, WriteOptions, js_number};
use song::Kind;
use song_edit::confine::workspace_dir_of;
use song_edit::library_forks::forks_whose_base_moved;
use song_edit::shared_folders::{
    LIBRARY_PINS_FILE, UNPINNED_VERSION, account_libraries, library_home_of, library_version_root, version_of_dir,
};
use song_edit::workspace_doc::{EditOpts, ws_edit};
use song_edit::workspace_file::{k7x_data, ws_exists, ws_read};

/// What this k7 runs: a version whose manifest `requires` more is refused.
const LIBRARY_LEVEL: f64 = 2.0;
const DRAFT: &str = "draft";

fn int(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Num(n)) if n.fract() == 0.0 && *n >= 1.0 => Some(*n),
        _ => None,
    }
}

/// The numbered versions this machine holds; a library in one folder is its version 1.
fn held_versions(h: &Host, name: &str) -> Vec<f64> {
    let home = library_home_of(&h.env, name);
    let Ok(list) = h.fs.list(&home) else {
        return Vec::new();
    };
    let dirs: Vec<String> = list
        .into_iter()
        .filter(|e| e.kind == Kind::Dir)
        .filter_map(|e| version_of_dir(&e.name).map(str::to_owned))
        .collect();
    if dirs.is_empty() {
        return vec![1.0];
    }
    let mut out: Vec<f64> = dirs.iter().filter_map(|d| d.parse::<f64>().ok()).collect();
    out.sort_by(f64::total_cmp);
    out
}

/// The level version `v` needs, from its `library.k7x`; a version with no manifest needs 1.
fn requires_of(h: &Host, name: &str, v: f64) -> f64 {
    let file = join(
        &library_version_root(&*h.fs, &h.env, name, &js_number(v)),
        "library.k7x",
    );
    let Ok(text) = ws_read(&*h.fs, &file) else {
        return 1.0;
    };
    match k7x_data(&file, &text) {
        Ok(d) if matches!(d.get("name"), Some(Value::Str(_))) && int(d.get("version")).is_some() => {
            int(d.get("requires")).unwrap_or(1.0)
        }
        _ => 1.0,
    }
}

fn newest_runnable(h: &Host, name: &str) -> f64 {
    held_versions(h, name)
        .into_iter()
        .filter(|v| requires_of(h, name, *v) <= LIBRARY_LEVEL)
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
        .unwrap_or(1.0)
}

/// The workspace's pins, name to version as written (`draft` or a number).
fn read_pins(h: &Host) -> (String, Vec<(String, String)>) {
    let abs = join(&workspace_dir_of(&h.env, &h.song_path), LIBRARY_PINS_FILE);
    let pins = ws_read(&*h.fs, &abs)
        .ok()
        .filter(|_| ws_exists(&*h.fs, &abs))
        .and_then(|t| k7x_data(&abs, &t).ok())
        .map(|d| match d {
            Value::Map(m) => m
                .into_iter()
                .filter_map(|(n, v)| match v {
                    Value::Str(s) if s == DRAFT => Some((n, s)),
                    Value::Num(x) if x.fract() == 0.0 && x >= 1.0 => Some((n, js_number(x))),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();
    (abs, pins)
}

fn pinned(pins: &[(String, String)], name: &str) -> String {
    pins.iter()
        .find(|(n, _)| n == name)
        .map_or_else(|| UNPINNED_VERSION.to_owned(), |(_, v)| v.clone())
}

fn library_names(h: &Host) -> Vec<String> {
    account_libraries(&*h.fs, &h.env, Some(&h.song_path))
        .into_iter()
        .map(|l| l.name)
        .filter(|n| n != "account")
        .collect()
}

/// `library versions` — each library: the version pinned here, and the ones this machine holds.
pub fn library_versions(_a: &str, h: &mut Host) -> Done {
    let (_, pins) = read_pins(h);
    let lines: Vec<String> = library_names(h)
        .iter()
        .map(|name| {
            let pin = pinned(&pins, name);
            let held: Vec<String> = held_versions(h, name)
                .into_iter()
                .map(|v| {
                    let r = requires_of(h, name, v);
                    if r > LIBRARY_LEVEL {
                        format!("{} (needs level {})", js_number(v), js_number(r))
                    } else {
                        js_number(v)
                    }
                })
                .collect();
            let newest = newest_runnable(h, name);
            let behind = match pin.parse::<f64>() {
                Ok(p) if newest > p => format!(" — {} is ready: library upgrade {name}", js_number(newest)),
                _ => String::new(),
            };
            let here = if held.is_empty() {
                "none".to_owned()
            } else {
                held.join(", ")
            };
            format!("@{name}: plays {pin}; here {here}{behind}")
        })
        .collect();
    if lines.is_empty() {
        h.log(["no library on this machine".to_owned()]);
    } else {
        h.log(lines);
    }
    Ok(())
}

/// `library upgrade <library> [version|draft]` — moves the pin; unsaid, to the newest version this k7 runs.
pub fn library_upgrade(a: &str, h: &mut Host) -> Done {
    let (abs, pins) = read_pins(h);
    let w = split_words(a);
    let names = library_names(h);
    let Some(name) = w.first().copied().filter(|n| names.iter().any(|x| x == n)) else {
        h.refuse([format!("library upgrade: name one of {}", names.join(", "))]);
        return Ok(());
    };
    let from = pinned(&pins, name);
    let to = match w.get(1).copied() {
        Some(DRAFT) => DRAFT.to_owned(),
        Some(want) => {
            let n = song_edit::js_text::str_to_number(want);
            if n.fract() != 0.0 || !n.is_finite() || n < 1.0 {
                h.refuse([format!("library upgrade: '{want}' is no version")]);
                return Ok(());
            }
            js_number(n)
        }
        None => js_number(newest_runnable(h, name)),
    };
    if to == from {
        h.log([format!("@{name} already plays {from}")]);
        return Ok(());
    }
    if let Ok(n) = to.parse::<f64>() {
        if !held_versions(h, name).contains(&n) {
            h.refuse([format!("@{name} {to} is not on this machine")]);
            return Ok(());
        }
        let r = requires_of(h, name, n);
        if r > LIBRARY_LEVEL {
            h.refuse([format!(
                "@{name} {to} needs k7 level {}; this k7 runs level {} — update k7 first",
                js_number(r),
                js_number(LIBRARY_LEVEL)
            )]);
            return Ok(());
        }
    }
    let value = to.parse::<f64>().map_or_else(|_| Value::Str(to.clone()), Value::Num);
    let opts = EditOpts {
        create: true,
        raw: false,
    };
    if let Err(e) = ws_edit(h.fs, &abs, opts, |doc| {
        Ok(doc.set_in(&[PathSeg::Key(name.to_owned())], &value, WriteOptions::default())?)
    }) {
        h.refuse([format!("library upgrade: {e}")]);
        return Ok(());
    }
    h.reload();
    let repo = workspace_dir_of(&h.env, &h.song_path);
    let moved = forks_whose_base_moved(&*h.fs, &h.env, &repo, name, &from, &to).unwrap_or_default();
    h.log(
        std::iter::once(format!("@{name}: {from} → {to}")).chain(
            moved
                .into_iter()
                .map(|f| format!("  {} extends {}, which changed", f.module, f.base)),
        ),
    );
    Ok(())
}
