//! The path model against node's own shims: every case of `vectors/paths.json` (made by `js/makeVectors.mts`)
//! replayed, Windows spellings included on any host; and a grep that holds 1020: `cfg!(windows)` in `src/` lives in `filter.rs` alone.

use k7_file_service::filter::{is_binary, project_app, skip};
use k7_file_service::libraries::{Version, is_library_name, version_of_dir};
use k7_file_service::paths::*;
use serde_json::Value;

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/paths.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn replays_every_vector_of_the_shims() {
    let all = vectors();
    assert!(all.len() >= 400, "{} cases", all.len());
    let mut seen = std::collections::BTreeSet::new();
    for c in &all {
        let f = c["fn"].as_str().unwrap();
        let a: Vec<&str> = c["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let out = &c["out"];
        let got = match (f, a.as_slice()) {
            ("resolve", [x, y]) => Value::from(resolve(x, y)),
            ("resolve", many) => Value::from(resolve_all(many)),
            ("join", parts) => Value::from(join(parts)),
            ("dirname", [x]) => Value::from(dirname(x)),
            ("basename", [x]) => Value::from(basename(x)),
            ("extname", [x]) => Value::from(extname(x)),
            ("normalize", [x]) => Value::from(normalize(x)),
            ("relative", [x, y]) => Value::from(relative(x, y)),
            ("is_within", [x, y]) => Value::from(is_within(x, y)),
            ("climbs_out", [x]) => Value::from(climbs_out(x)),
            ("is_binary", [x]) => Value::from(is_binary(x)),
            ("skip", [x]) => Value::from(skip(x)),
            ("project_app", [x]) => project_app(x).map_or(Value::Null, Value::from),
            ("is_library_name", [x]) => Value::from(is_library_name(x)),
            ("version_of_dir", [x]) => match version_of_dir(x) {
                None => Value::Null,
                Some(Version::Draft) => Value::from("draft"),
                Some(Version::Number(n)) => Value::from(n),
            },
            other => panic!("a vector of a function the crate lacks: {other:?}"),
        };
        assert_eq!(&got, out, "{f} {a:?}");
        seen.insert(f.to_string());
    }
    assert_eq!(seen.len(), 14, "{seen:?}");
}

#[test]
fn a_windows_root_is_ordinary_data_on_any_host() {
    assert_eq!(resolve("C:\\Users\\x", "..\\y"), "C:/Users/y");
    assert!(is_within("C:/Work", "c:/work/Songs"));
    assert!(!is_within("/work", "/WORK/songs"));
    assert!(!is_within("C:/work", "C:/workshop"));
    // A drive path on a POSIX-rooted base escapes, as node answers it.
    assert!(!is_within("/home/x", &resolve("/home/x", "C:/y")));
}

/// Every `.rs` below `dir`, entries/ included (1234).
fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            sources(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn the_one_cfg_windows_is_the_error_code_s() {
    let mut files = Vec::new();
    sources(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        &mut files,
    );
    let mut found = Vec::new();
    for p in files {
        let text = std::fs::read_to_string(&p).unwrap();
        if text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains("cfg!(windows)") || l.contains("cfg(windows)"))
        {
            found.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert_eq!(found, ["filter.rs"]);
}
