//! The path model against node's own shims: every case of `vectors/paths.json` (made by `js/makeVectors.mts`)
//! replayed, Windows spellings included on any host; and a grep that holds 1020: `cfg!(windows)` in `src/` lives in `filter.rs` alone.

use k7_file_service::paths::*;

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
