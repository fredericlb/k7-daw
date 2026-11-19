//! The listings against the TypeScript's own functions (1460): every scenario of `vectors/listings.json` (made by
//! `js/makeVectors.mts`) built on disk, the five listings written and compared as text, so the keys' order is held too.
//! A scenario this host cannot build (a name the disk refuses, a link it will not make) is skipped, never assumed.

use crate::support;

use k7_file_service::listings::{Listing, Remote, list};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::Value;
use support::{Scratch, link, link_file};

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/listings.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Builds a scenario's entries under `s`; false where this host refuses one of them.
fn build(s: &Scratch, entries: &[Value]) -> bool {
    for e in entries {
        let p = e["p"].as_str().unwrap();
        // Windows takes neither a backslash in a name nor a trailing dot.
        if cfg!(windows) && (p.contains('\\') || p.split('/').any(|n| n.ends_with('.') && n != "."))
        {
            return false;
        }
        let at = s.raw(p);
        let parent = std::path::Path::new(&at).parent().unwrap().to_path_buf();
        if std::fs::create_dir_all(&parent).is_err() {
            return false;
        }
        let made = if let Some(to) = e["link"].as_str() {
            link(&s.raw(to), &at).is_ok()
        } else if let Some(to) = e["file_link"].as_str() {
            link_file(&s.raw(to), &at).is_ok()
        } else if p.ends_with('/') {
            std::fs::create_dir_all(&at).is_ok()
        } else {
            let body = match e["size"].as_u64() {
                Some(n) => "x".repeat(usize::try_from(n).unwrap()),
                None => e["t"].as_str().unwrap_or("").to_string(),
            };
            std::fs::write(&at, body).is_ok()
        };
        if !made {
            return false;
        }
    }
    true
}

const LISTINGS: [(&str, Listing); 5] = [
    ("playlists", Listing::Playlists),
    ("references", Listing::References),
    ("chains", Listing::LibraryChains),
    ("assemblies", Listing::LibraryAssemblies),
    ("levels", Listing::ModuleLibraries),
];

/// `json` without its engine row, which only the scenarios about the scan keep.
fn without_engine(json: &str) -> String {
    match json.rfind("{\"ns\":\"engine\"") {
        Some(at) => {
            let head = json[..at].strip_suffix(',').unwrap_or(&json[..at]);
            format!("{head}]}}")
        }
        None => json.to_string(),
    }
}

#[test]
fn every_scenario_of_the_typescripts_own_listings_is_written_the_same() {
    let rows = vectors();
    let (mut compared, mut skipped) = (0, 0);
    for row in &rows {
        let name = row["name"].as_str().unwrap();
        let s = Scratch::new("listings-vector");
        let entries = row["entries"].as_array().unwrap();
        s.dir("ws");
        if !build(&s, entries) {
            skipped += 1;
            continue;
        }
        let home = s.home();
        let space = Space::open(&home, &s.path("ws"));
        let with_engine = name.contains("engine");
        for (key, which) in LISTINGS {
            let got = list(&space, which, &Remote::default(), &Limits::default());
            match row["answers"][key].as_str() {
                Some(want) => {
                    let got = got.unwrap_or_else(|d| panic!("{name} / {key}: declined {d:?}"));
                    let text = if with_engine || which != Listing::ModuleLibraries {
                        got.json
                    } else {
                        without_engine(&got.json)
                    };
                    assert_eq!(text, want, "{name} / {key}");
                    compared += 1;
                }
                // The TypeScript throws: an assembly walk is declined, and a level's unlistable folder costs only that folder (1423).
                None if which == Listing::LibraryAssemblies => {
                    assert_eq!(got.unwrap_err(), Declined::WouldThrow, "{name} / {key}");
                    compared += 1;
                }
                None => {
                    let got = got.unwrap_or_else(|d| panic!("{name} / {key}: declined {d:?}"));
                    assert!(!got.unlisted.is_empty(), "{name} / {key}");
                    compared += 1;
                }
            }
        }
    }
    println!("{compared} listings compared, {skipped} scenarios skipped");
    assert!(compared >= 120 || cfg!(windows), "{compared} compared");
    if cfg!(unix) {
        assert_eq!(skipped, 0);
    }
}
