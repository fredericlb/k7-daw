//! Every archetype created natively is `e2e/specs/conformance/scaffolds.json` byte for byte: each file's sha256, ids blanked, and what lands beside the project.

use std::collections::BTreeMap;

use k7_file_service::edits::project_create::create_project;
use k7_file_service::libraries::Home;
use k7_file_service::space::Space;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::support::Scratch;

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every `[0-9a-f]{8}-…-[0-9a-f]{12}` as `<id>`, as the spec's `blankIds` does.
fn blank_ids(text: &str) -> String {
    let b = text.as_bytes();
    let shape = [8usize, 4, 4, 4, 12];
    let mut out = String::new();
    let mut i = 0;
    'scan: while i < b.len() {
        let mut at = i;
        for (n, len) in shape.iter().enumerate() {
            if n > 0 {
                if b.get(at) != Some(&b'-') {
                    out.push(text[i..].chars().next().unwrap());
                    i += text[i..].chars().next().unwrap().len_utf8();
                    continue 'scan;
                }
                at += 1;
            }
            if !b
                .get(at..at + len)
                .is_some_and(|s| s.iter().all(u8::is_ascii_hexdigit))
            {
                out.push(text[i..].chars().next().unwrap());
                i += text[i..].chars().next().unwrap().len_utf8();
                continue 'scan;
            }
            at += len;
        }
        out.push_str("<id>");
        i = at;
    }
    out
}

fn files_under(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for p in names {
        if p.is_dir() {
            files_under(&p, base, out);
        } else {
            out.push(
                p.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn every_archetype_is_written_as_scaffolds_json_holds_it() {
    let manifest = format!(
        "{}/../../../../../e2e/specs/conformance/scaffolds.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let want: Value = serde_json::from_str(&std::fs::read_to_string(manifest).unwrap()).unwrap();
    let code = format!("{}/../../../..", env!("CARGO_MANIFEST_DIR"));
    // One workspace for all five, as the spec's: what an earlier shape wrote beside its folder is not a later one's.
    let s = Scratch::new("sc-all");
    let ws = s.dir("ws");
    for shape in ["arrangement", "mix", "video", "live", "live-video"] {
        let mut before = vec![];
        files_under(
            std::path::Path::new(&ws),
            std::path::Path::new(&ws),
            &mut before,
        );
        let home = Home {
            studio: s.path("home"),
            libraries: s.path("libraries"),
        };
        let space = Space::open(&home, &ws);
        let mut n = 0;
        let mut mint = || {
            n += 1;
            format!("00000000-0000-4000-8000-{n:012}")
        };
        let name = format!("{shape}.k7");
        let made = create_project(&home, &code, &space, shape, &name, &mut mint)
            .unwrap_or_else(|e| panic!("{shape}: {e}"));
        assert_eq!(made.answer["project"], name.as_str(), "{shape}");
        let (root, project) = (
            std::path::PathBuf::from(&ws),
            std::path::PathBuf::from(&ws).join(&name),
        );
        let mut inside = vec![];
        files_under(&project, &project, &mut inside);
        let files: BTreeMap<String, String> = inside
            .iter()
            .map(|rel| {
                let bytes = std::fs::read(project.join(rel)).unwrap();
                let binary = [".mp4", ".wav", ".png"].iter().any(|e| rel.ends_with(e));
                let sum = if binary {
                    hex(&bytes)
                } else {
                    hex(blank_ids(&String::from_utf8(bytes).unwrap()).as_bytes())
                };
                (rel.clone(), sum)
            })
            .collect();
        let mut all = vec![];
        files_under(&root, &root, &mut all);
        let outside: Vec<String> = all
            .into_iter()
            .filter(|f| {
                !f.starts_with(&format!("{name}/")) && !before.contains(f) && !f.contains(".k7/")
            })
            .collect();
        let held = &want[shape];
        let held_files: BTreeMap<String, String> = held["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(files, held_files, "{shape}");
        let held_out: Vec<String> = held["workspace"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(outside, held_out, "{shape}");
    }
}
