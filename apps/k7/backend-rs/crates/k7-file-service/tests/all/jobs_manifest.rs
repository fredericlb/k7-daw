//! jobs_manifest.rs — `parse_manifest` and `move_into` against the TypeScript's answers, and the departures of B2 (2320, DEPARTS 2320b).
//! A reader gets wrong that a vector naming a file outside the staging, a relative path or two files of one name is the TypeScript's
//! answer kept as it was: the native one is the sentence beside it here, and the departures are exactly these three.

use crate::support::jobs::{Tmp, section, vectors};
use k7_file_service::jobs::manifest::{OUTSIDE, move_into, parse_manifest};
use serde_json::{Value, json};

#[test]
fn every_manifest_of_the_typescript_is_read_alike() {
    let rows = section(&vectors(), "manifest");
    assert!(rows.len() >= 16);
    for c in &rows {
        let got = match parse_manifest(c["stdout"].as_str().unwrap()) {
            Ok(v) => json!({ "ok": v }),
            Err(e) => json!({ "error": e }),
        };
        assert_eq!(got, c["out"], "{:?}", c["stdout"]);
    }
}

#[test]
fn the_javascript_trim_and_line_rules_hold() {
    // U+FEFF is white space to JavaScript's trim; U+0085 is not.
    assert_eq!(
        parse_manifest("\u{feff}{\"mix\":\"a\"}\u{feff}").unwrap(),
        json!({ "mix": "a" })
    );
    assert!(parse_manifest("\u{85}{\"mix\":\"a\"}").is_err());
    // A `\r` before a newline goes with the separator.
    assert_eq!(
        parse_manifest("x\r\n{\"mix\":\"a\"}\r\n").unwrap(),
        json!({ "mix": "a" })
    );
}

/// A case's files on disk under a folder of its own; `{s}` is its staging and `{base}` the folder.
fn spell(v: &Value, base: &str) -> Value {
    serde_json::from_str(
        &v.to_string()
            .replace("{s}", &format!("{base}/s"))
            .replace("{base}", base),
    )
    .unwrap()
}

fn listing(dir: &str) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        std::fs::read_to_string(e.path()).unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    rows.sort();
    rows
}

/// The vectors whose native answer is the sentence of B2 and not the TypeScript's.
const DEPARTS: [&str; 3] = [
    "a reply naming a file outside the staging",
    "a relative path",
    "two stems of one name",
];

#[test]
fn every_move_of_the_typescript_is_made_alike_but_the_three_that_depart() {
    let rows = section(&vectors(), "move_into");
    assert!(rows.len() >= 17);
    let mut departed = Vec::new();
    for c in &rows {
        let name = c["name"].as_str().unwrap();
        let tmp = Tmp::new("jobsmove");
        let base = tmp.at("").trim_end_matches('/').to_owned();
        for f in c["files"].as_array().unwrap() {
            let size = f["size"].as_u64();
            tmp.write(
                f["p"].as_str().unwrap(),
                &"x".repeat(size.unwrap_or(0) as usize),
            );
            if size.is_none() {
                tmp.write(f["p"].as_str().unwrap(), f["t"].as_str().unwrap_or(""));
            }
        }
        let staging = format!("{base}/s");
        let dest = format!("{base}/dest/out");
        let got = move_into(&spell(&c["reply"], &base), &staging, &dest);
        let want = &c["out"];
        // A backslash separates, as the path model reads it; the TypeScript's copy on a Unix front read it as a letter (ENOENT).
        if name == "a Windows-spelt path to a file of the staging" {
            departed.push(name);
            assert_eq!(
                got.as_ref().map(|m| m.mix.as_str()),
                Ok("mix.wav"),
                "{name}"
            );
            continue;
        }
        if name == "a drive path" {
            departed.push(name);
            assert_eq!(got, Err(OUTSIDE.to_owned()), "{name}");
            continue;
        }
        if DEPARTS.contains(&name) {
            departed.push(name);
            assert!(got.is_err(), "{name}: {got:?}");
            assert!(
                listing(&dest).is_empty(),
                "{name}: a refusal copies nothing"
            );
            continue;
        }
        match (
            &got,
            want.get("ok"),
            want.get("error").and_then(Value::as_str),
        ) {
            (Ok(m), Some(ok), None) => {
                assert_eq!(m.mix, ok["mix"].as_str().unwrap(), "{name}");
                let stems: Vec<Value> = m
                    .stems
                    .iter()
                    .map(|(n, f)| json!({ "name": n, "file": f }))
                    .collect();
                assert_eq!(Value::Array(stems), ok["stems"], "{name}");
                let made: Vec<(String, String)> = c["made"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| (r[0].as_str().unwrap().into(), r[1].as_str().unwrap().into()))
                    .collect();
                assert_eq!(listing(&dest), made, "{name}");
            }
            // The sentence of a copy that failed is the platform's for a directory, and Node's own for a missing file.
            (Err(e), None, Some(w))
                if w.starts_with("ENOENT") || w.starts_with("the engine's machine") =>
            {
                assert_eq!(e, &w.replace("{base}", &base), "{name}");
            }
            (Err(_), None, Some(_)) => {}
            other => panic!("{name}: {other:?} against {want}"),
        }
    }
    assert_eq!(departed.len(), DEPARTS.len() + 2);
    assert!(DEPARTS.iter().all(|d| departed.contains(d)));
}

#[test]
fn a_file_outside_the_staging_or_through_a_link_out_of_it_is_refused_and_nothing_is_copied() {
    let tmp = Tmp::new("jobsmovelink");
    tmp.write("s/mix.wav", "m");
    tmp.write("elsewhere/secret.wav", "secret");
    let (staging, dest) = (tmp.at("s"), tmp.at("dest"));
    let outside = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("elsewhere/secret.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(outside, Err(OUTSIDE.to_owned()));
    assert!(
        !std::path::Path::new(&dest).exists(),
        "the mix was not copied before the refusal"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(tmp.at("elsewhere/secret.wav"), tmp.at("s/link.wav")).unwrap();
        let linked = move_into(
            &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/link.wav") }] }),
            &staging,
            &dest,
        );
        assert_eq!(linked, Err(OUTSIDE.to_owned()));
    }
    // Two files of one name are refused whole, mix against stem included.
    tmp.write("s/a/mix.wav", "other");
    let twice = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/a/mix.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(
        twice,
        Err("the engine's machine named two files 'mix.wav'".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn a_destination_that_is_a_link_is_replaced_never_followed_on_unix_only() {
    let tmp = Tmp::new("jobsmovedest");
    tmp.write("s/mix.wav", "new");
    tmp.write("elsewhere/target.wav", "kept");
    tmp.mkdir("dest");
    std::os::unix::fs::symlink(tmp.at("elsewhere/target.wav"), tmp.at("dest/mix.wav")).unwrap();
    let moved = move_into(
        &json!({ "mix": tmp.at("s/mix.wav") }),
        &tmp.at("s"),
        &tmp.at("dest"),
    )
    .unwrap();
    assert_eq!(moved.mix, "mix.wav");
    assert_eq!(
        std::fs::read_to_string(tmp.at("elsewhere/target.wav")).unwrap(),
        "kept"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.at("dest/mix.wav")).unwrap(),
        "new"
    );
}

#[test]
fn names_that_differ_only_by_case_or_trailing_dots_and_spaces_are_one_name() {
    let tmp = Tmp::new("jobsmovecase");
    tmp.write("s/mix.wav", "m");
    tmp.write("s/A.wav", "a");
    tmp.write("s/a.wav", "b");
    let (staging, dest) = (tmp.at("s"), tmp.at("dest"));
    for second in ["a.wav", "A.WAV", "a.wav.", "a.wav "] {
        let moved = move_into(
            &json!({ "mix": tmp.at("s/mix.wav"), "stems": [
                { "name": "x", "file": tmp.at("s/A.wav") },
                { "name": "y", "file": format!("{}/{second}", tmp.at("s")) },
            ] }),
            &staging,
            &dest,
        );
        assert_eq!(
            moved,
            Err(format!("the engine's machine named two files '{second}'"))
        );
        assert!(!std::path::Path::new(&dest).exists(), "{second}");
    }
    // Different names stay different.
    let two = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/A.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(two.unwrap().stems, [("x".to_owned(), "A.wav".to_owned())]);
}
