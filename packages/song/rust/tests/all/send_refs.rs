// send_refs.rs — the four ports the file service's send needs, replayed from ports.json, which the TypeScript's own functions made.
// A reader would get wrong that the corpus half is an ignored gate: it needs the staged projects and the views' dumps.

use crate::support::{mem, mem_world_roots};
use serde_json::Value;
use song::clips::{audition_hosts, audition_timeline, material_ref_of};
use song::doc::ProjectDoc;
use song::send_refs::{names_external_plugin, pattern_files, playlist_records};
use song::{Cache, DiskSource, Roots};
use std::path::PathBuf;
use std::sync::OnceLock;

fn ports() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/all/ports.json")).expect("ports.json");
        serde_json::from_str(&text).expect("ports.json is JSON")
    })
}

/// The files of a case laid under the project `/p/ws/proj`, the way the lane tests lay theirs.
fn laid_under(project: &str, files: &Value) -> Vec<(String, String)> {
    files
        .as_object()
        .expect("files")
        .iter()
        .map(|(rel, text)| (format!("{project}/{rel}"), text.as_str().expect("text").to_owned()))
        .collect()
}

fn source_of(files: &[(String, String)]) -> song::MemSource {
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    mem(&refs)
}

#[test]
fn a_project_names_an_external_plugin_as_the_typescript_says() {
    let cases = ports()["names_external_plugin"]["small"].as_array().expect("cases");
    assert!(cases.len() >= 7);
    for c in cases {
        let src = source_of(&laid_under("ws/proj", &c["files"]));
        let got = names_external_plugin(&src, &mut Cache::new(), &mem_world_roots());
        assert_eq!(Value::Bool(got), c["out"], "{}", c["name"]);
    }
}

#[test]
fn the_pattern_files_of_a_project_are_the_typescript_s() {
    let cases = ports()["pattern_files"]["small"].as_array().expect("cases");
    assert!(cases.len() >= 6);
    for c in cases {
        let src = source_of(&laid_under("ws/proj", &c["files"]));
        let got = pattern_files(&src, &mut Cache::new(), &mem_world_roots());
        assert_eq!(serde_json::json!(got), c["out"], "{}", c["name"]);
    }
}

#[test]
fn the_records_a_desk_names_are_the_typescript_s() {
    let cases = ports()["playlist_records"]["small"].as_array().expect("cases");
    assert!(cases.len() >= 5);
    for c in cases {
        let src = source_of(&laid_under("ws", &c["files"]));
        let roots = Roots {
            project: "/p/ws/p".into(),
            ..mem_world_roots()
        };
        let got = playlist_records(&src, &mut Cache::new(), &roots, "/p/ws");
        assert_eq!(serde_json::json!(got), c["out"], "{}", c["name"]);
    }
}

#[test]
fn an_auditioned_timeline_is_the_one_the_typescript_makes() {
    let cases = ports()["audition_timeline"].as_array().expect("cases");
    assert!(cases.len() >= 4);
    for c in cases {
        let id = c["id"].as_str().expect("id");
        let doc: ProjectDoc = serde_json::from_value(c["doc"].clone()).expect("a compiled document");
        assert_eq!(
            serde_json::json!(audition_hosts(&doc, id)),
            c["hosts"],
            "{} {id}",
            c["name"]
        );
        let got = serde_json::to_value(audition_timeline(doc, id)).expect("a value");
        assert_eq!(js_numbers(&got), js_numbers(&c["out"]), "{} {id}", c["name"]);
    }
}

#[test]
fn a_material_is_split_on_the_last_at_before_a_number() {
    assert_eq!(material_ref_of("verse>kick@3"), "kick");
    assert_eq!(material_ref_of("kick@a"), "kick@a");
    assert_eq!(material_ref_of("a@b@2"), "a@b");
    assert_eq!(material_ref_of("@5"), "@5");
}

fn js_numbers(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15 => Value::from(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(js_numbers).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), js_numbers(x))).collect()),
        _ => v.clone(),
    }
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_corpus_agrees_with_the_typescript() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let read =
        |p: PathBuf| -> Value { serde_json::from_str(&std::fs::read_to_string(p).expect("a file")).expect("JSON") };
    let mut failures = vec![];
    let mut count = 0;
    for p in read(staged.join("manifest.json"))["projects"]
        .as_array()
        .expect("projects")
    {
        let name = p["project"].as_str().expect("name");
        let theirs = read(dumps.join(format!("{}.json", name.replace('/', "__"))));
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("roots");
        let workspace = roots.workspace.clone().expect("a workspace");
        let held = |k: &str| {
            ports()[k]["corpus"]
                .as_array()
                .expect("corpus")
                .iter()
                .find(|c| c["project"] == name)
                .map(|c| c["out"].clone())
        };
        let (mounts, files) = roots.mounts().expect("mounts");
        let (mut cache, src) = (Cache::new(), DiskSource::with_files(&mounts, &files).expect("a source"));
        let got = [
            (
                "names_external_plugin",
                Value::Bool(names_external_plugin(&src, &mut cache, &roots)),
            ),
            (
                "pattern_files",
                serde_json::json!(pattern_files(&src, &mut cache, &roots)),
            ),
            (
                "playlist_records",
                serde_json::json!(playlist_records(&src, &mut cache, &roots, &workspace)),
            ),
        ];
        for (k, v) in got {
            match held(k) {
                Some(want) if want == v => count += 1,
                other => failures.push(format!("{name} {k}: got {v}, the TypeScript says {other:?}")),
            }
        }
    }
    assert!(count >= 3 * 170, "{count} answers compared: it proves nothing");
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
