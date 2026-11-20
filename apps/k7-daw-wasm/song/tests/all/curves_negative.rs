// curves_negative.rs — known-good and known-bad curve folders: exact file sets and values, on disk and through the bundle it made.
// A reader would get wrong that this needs no TypeScript: curveDump.ts stages the `ts` cases and curves_stage.rs adds its side.

use crate::support;

use serde_json::{Value, json};
use song::curves::{CurvesRead, read_curves};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Severity};
use support::Tmp;

struct Neg {
    name: String,
    category: String,
    ts: bool,
    mem: bool,
    files: Vec<(String, Vec<u8>)>,
    expect: Value,
}

fn bytes_of(v: &Value) -> Vec<u8> {
    match v.get("hex").and_then(Value::as_str) {
        Some(hex) => (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
            .collect(),
        None => v.as_str().expect("text").as_bytes().to_vec(),
    }
}

fn cases() -> Vec<Neg> {
    let all: Vec<Value> = serde_json::from_str(include_str!("negative_curves.json")).expect("negative_curves.json");
    all.iter()
        .map(|c| Neg {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().expect("mem"),
            files: c["files"]
                .as_object()
                .expect("files")
                .iter()
                .map(|(rel, v)| (rel.clone(), bytes_of(v)))
                .collect(),
            expect: c["expect"].clone(),
        })
        .collect()
}

fn strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// What a read answers, as the case states it: the ids, the finding files and every curve as JSON.
fn shape(read: &CurvesRead) -> Value {
    let mut files: Vec<String> = read.findings.iter().map(|f| f.file.clone()).collect();
    files.sort();
    let curves: serde_json::Map<String, Value> = read
        .curves
        .iter()
        .map(|(id, c)| (id.clone(), serde_json::to_value(&**c).expect("json")))
        .collect();
    json!({ "files": files, "curves": curves })
}

fn check(case: &Neg, got: &Value) {
    assert_eq!(
        strings(&got["files"]),
        strings(&case.expect["files"]),
        "{} files",
        case.name
    );
    let mut ids: Vec<String> = got["curves"].as_object().expect("curves").keys().cloned().collect();
    ids.sort();
    assert_eq!(ids, strings(&case.expect["ids"]), "{} ids", case.name);
    for (id, pointers) in case.expect["values"].as_object().into_iter().flatten() {
        for (pointer, want) in pointers.as_object().into_iter().flatten() {
            let have = got["curves"][id].pointer(pointer).cloned().unwrap_or(Value::Null);
            let same = match (have.as_f64(), want.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                _ => have == *want,
            };
            assert!(same, "{} {id} {pointer}: {have} is not {want}", case.name);
        }
    }
}

#[test]
fn every_known_curve_folder_reads_to_its_exact_file_set_on_disk_and_through_its_bundle() {
    let t = Tmp::new("curves-negative");
    let all = cases();
    for case in &all {
        let project = t.project(&case.name, &case.files);
        let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
        let rec = Recorder::new(&disk);
        let read = read_curves(&rec, &mut Cache::new(), &project).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        assert!(
            read.findings.iter().all(|f| f.severity == Severity::Warning),
            "{}",
            case.name
        );
        let on_disk = shape(&read);
        check(case, &on_disk);
        if case.mem {
            let bundle = rec.into_bundle(std::slice::from_ref(&project));
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            let again = read_curves(&mem, &mut Cache::new(), &project)
                .unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
            assert_eq!(shape(&again), on_disk, "{} read through its bundle", case.name);
        }
    }
    let categories = |c: &str| all.iter().filter(|n| n.category == c).count();
    for (category, min) in [
        ("good", 7),
        ("normalised", 5),
        ("condition", 8),
        ("refused", 30),
        ("hostile", 1),
        ("syntax", 1),
        ("non-utf8", 1),
    ] {
        assert!(
            categories(category) >= min,
            "category {category}: {} cases",
            categories(category)
        );
    }
    assert!(all.len() >= 30 && all.iter().filter(|c| c.ts).count() >= 20);
}

// decision 14: a file that is not UTF-8 is a finding, held by its length and flagged binary, so the bundle replays it (745).
#[test]
fn a_file_that_is_not_utf8_is_a_finding_and_the_bundle_replays_it() {
    let t = Tmp::new("curves-utf8");
    let project = t.project(
        "p",
        &[
            ("curves/bad.k7x".to_owned(), vec![0x74, 0xff, 0xfe]),
            ("curves/ok.k7x".to_owned(), b"type(\"lfo\")\n".to_vec()),
        ],
    );
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let rec = Recorder::new(&disk);
    let read = read_curves(&rec, &mut Cache::new(), &project).expect("reads");
    assert_eq!(read.curves.len(), 1);
    assert_eq!(read.findings.len(), 1);
    let mem = MemSource::from_bundle(rec.into_bundle(std::slice::from_ref(&project))).expect("a bundle");
    // The bundle says the file is not UTF-8, so it reads as the disk read it (decision 745).
    let again = read_curves(&mem, &mut Cache::new(), &project).expect("replays");
    assert_eq!(again.curves.len(), 1);
    assert_eq!(again.findings, read.findings);
}
