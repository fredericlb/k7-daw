//! The journal against the TypeScript's own functions (1721, 1723, 1724, 1727): every case of `vectors/journal.json`, made by
//! `js/journalVectors.mts` — the bytes `JSON.stringify` prints, the shape that is torn, the store's name, the text hash.

use crate::support;

use k7_file_service::journal::format::{parse, print};
use k7_file_service::journal::store::store_for;
use k7_file_service::libraries::Home;
use k7_file_service::paths::basename;
use k7_file_service::text_hash::text_hash;
use serde_json::Value;

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/journal.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn prints_the_bytes_json_stringify_prints_and_tears_what_the_typescript_tears() {
    let all = vectors();
    let rows = all["journals"].as_array().unwrap();
    assert!(rows.len() >= 400, "{} journals", rows.len());
    let (mut read, mut torn) = (0, 0);
    for r in rows {
        let text = r["in"].as_str().unwrap();
        match (parse(text), r["out"].as_str()) {
            (Ok(j), Some(out)) => {
                assert_eq!(print(&j), out, "{text}");
                // A printed journal reads again and prints the same.
                assert_eq!(print(&parse(out).unwrap()), out);
                read += 1;
            }
            (Err(_), None) => torn += 1,
            (got, want) => panic!(
                "{text}\n  native {:?}\n  typescript {want:?}",
                got.map(|j| print(&j))
            ),
        }
    }
    assert!(read >= 300 && torn >= 50, "{read} read, {torn} torn");
}

#[test]
fn names_the_store_as_history_store_for_does() {
    let home = Home {
        studio: "/h".into(),
        libraries: "/l".into(),
        admin: false,
    };
    let rows = vectors()["store"].as_array().unwrap().clone();
    for r in &rows {
        let dir = r["dir"].as_str().unwrap();
        assert_eq!(
            basename(&store_for(&home, dir)),
            r["name"].as_str().unwrap(),
            "{dir:?}"
        );
    }
    // A canonical spelling hashes elsewhere, and a drive's case is kept (1723).
    let name = |d: &str| basename(&store_for(&home, d));
    assert_ne!(
        name("/var/folders/zz/T/x.k7"),
        name("/private/var/folders/zz/T/x.k7")
    );
    assert_ne!(name("C:/Users/a/x.k7"), name("c:/Users/a/x.k7"));
    assert_eq!(name("C:/Users/a/x.k7"), name("C:\\Users\\a\\x.k7"));
}

#[test]
fn hashes_text_as_text_hash_does() {
    let rows = vectors()["hash"].as_array().unwrap().clone();
    assert!(rows.len() >= 480);
    for r in &rows {
        let text = match r["text"].as_str() {
            Some(t) => t.to_string(),
            None => r["unit"]
                .as_str()
                .unwrap()
                .repeat(r["times"].as_u64().unwrap() as usize),
        };
        assert_eq!(text_hash(&text), r["out"].as_str().unwrap(), "{:.40}", text);
    }
}

/// The fixture of `js/journalVectors.mts`, made afresh under `ws`.
fn fixture(s: &support::Scratch) {
    let _ = std::fs::remove_dir_all(s.path("ws"));
    for (p, c) in [
        ("song.k7/a.k7x", "disk\n"),
        ("song.k7/empty.k7x", ""),
        ("song.k7/bom.k7x", "\u{feff}bom\n"),
        ("song.k7/sub/x.k7x", "x\n"),
        ("notes.txt", "notes\n"),
        ("_modules/m.k7x", "mod\n"),
        ("song.k7/conf.yaml", "a: 1\n"),
    ] {
        s.file(&format!("ws/{p}"), c);
    }
    let wav = s.path("ws/song.k7/r.wav");
    std::fs::write(wav, [255u8, 0, 1]).unwrap();
}

#[test]
fn judges_put_file_as_the_typescripts_own_route_does_1760() {
    use k7_file_service::journal::put::{PutPlan, plan_put};
    use k7_file_service::space::Space;
    use k7_file_service::tree::Limits;
    let s = support::Scratch::new("jv-put");
    let home = s.home(false);
    let rows = vectors()["put"].as_array().unwrap().clone();
    assert!(rows.len() >= 400, "{} cases", rows.len());
    let (mut writes, mut refusals, mut declined, mut shim) = (0, 0, 0, 0);
    for r in &rows {
        fixture(&s);
        let (path, content) = (r["path"].as_str().unwrap(), r["content"].as_str().unwrap());
        let base = r["base"].as_str();
        let (status, body) = (r["status"].as_u64().unwrap() as u16, &r["body"]);
        let space = Space::open(&home, &s.path("ws"));
        let got = plan_put(&space, path, content, base, &Limits::default());
        let error = body["error"].as_str().unwrap_or("");
        // A failure of the write itself is the write's to find; the plan answers `Write` for it.
        let fs_failure =
            error.starts_with("the file could not be changed") || error == "no such file or folder";
        let tag = format!("{path:?} content {content:?} base {base:?}");
        match got {
            PutPlan::Write { .. } => {
                assert!(
                    status == 200 || fs_failure,
                    "{tag}: planned a write, the route answered {status} {body}"
                );
                writes += 1;
            }
            // The shim does not hide ENOTDIR on a stat as node does.
            PutPlan::Refused {
                status: 400,
                body: b,
            } if path.contains("a.k7x/below")
                && base.is_some()
                && error.ends_with("(EEXIST)")
                && b["error"] == "the file could not be changed (ENOTDIR)" =>
            {
                shim += 1;
            }
            PutPlan::Refused {
                status: st,
                body: b,
            } => {
                assert_eq!((st, &b), (status, body), "{tag}");
                refusals += 1;
            }
            PutPlan::Declined(d) => {
                eprintln!("declined {tag}: {}", d.as_str());
                declined += 1;
            }
        }
    }
    assert_eq!(
        (declined, shim),
        // Windows answers NotFound below a file, which the plan already accepts: nothing is excused there.
        (0, if cfg!(windows) { 0 } else { 21 }),
        "the referee's own difference is named and counted"
    );
    assert!(
        writes >= 100 && refusals >= 200,
        "{writes} writes, {refusals} refusals"
    );
}
