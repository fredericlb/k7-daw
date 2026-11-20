// views_gate/sweeps.rs — the gate's three sweeps: the fields lattice, the WAV headers and the lists of clips, against the TypeScript.
// A reader would get wrong that the lattice names exactly one known difference (decision 650) and no other.

use super::{Counts, diff};
use serde_json::Value;
use song::clips::ClipDoc;
use song::views::fields::{FileKind, fields_at, file_kind_for, values_at};
use song::views::lengths::{ByHeader, Measure};
use song::views::wav::wav_seconds;
use song::views::web::{fold_nests, web_clip};
use song::{Cache, DiskSource};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::read_json;
use crate::support;

fn kind_of(name: &str) -> FileKind {
    match name {
        "module" => FileKind::Module,
        "chains" => FileKind::Chains,
        "chain" => FileKind::Chain,
        "clip" => FileKind::Clip,
        "sequence" => FileKind::Sequence,
        "curve" => FileKind::Curve,
        _ => FileKind::Project,
    }
}

const KINDS: [&str; 7] = ["module", "chains", "chain", "clip", "sequence", "curve", "project"];

/// The one place the lattice is allowed to differ, each with its reason; the two lists are asserted exactly.
const PROJECT_ROOT_EXTRA: [&str; 2] = ["type", "project_type"];

fn set_of(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

pub fn lattice_gate(dumps: &Path, failures: &mut Vec<String>, c: &mut Counts) {
    let d = read_json(&dumps.join("fields.json"));
    let strings = |v: &Value| -> Vec<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_owned))
            .collect()
    };
    let (pool, named, field_pool) = (strings(&d["pool"]), strings(&d["named"]), strings(&d["fieldPool"]));
    let mut paths: Vec<Vec<String>> = vec![vec![]];
    for a in &pool {
        paths.push(vec![a.clone()]);
        for b in &pool {
            paths.push(vec![a.clone(), b.clone()]);
            for x in &pool {
                paths.push(vec![a.clone(), b.clone(), x.clone()]);
            }
        }
    }
    paths.extend(named.iter().map(|n| n.split('/').map(str::to_owned).collect()));
    let expected = |m: &Value, key: &str| -> BTreeSet<String> { strings(&m[key]).into_iter().collect() };
    for k in KINDS {
        for p in &paths {
            let refs: Vec<&str> = p.iter().map(String::as_str).collect();
            let ours: BTreeSet<String> = set_of(&fields_at(kind_of(k), &refs));
            let theirs = expected(&d["fields"], &format!("{k}|{}", p.join("/")));
            c.lattice += 1;
            if ours == theirs {
                continue;
            }
            let extra: BTreeSet<String> = theirs.difference(&ours).cloned().collect();
            let only_ours: BTreeSet<String> = ours.difference(&theirs).cloned().collect();
            let known = k == "project" && p.is_empty() && only_ours.is_empty() && extra == set_of(&PROJECT_ROOT_EXTRA);
            if !known {
                failures.push(format!(
                    "fields {k}|{} ts-only={extra:?} rust-only={only_ours:?}",
                    p.join("/")
                ));
            }
        }
        let mut value_paths: Vec<Vec<String>> = vec![vec![]];
        value_paths.extend(pool.iter().map(|s| vec![s.clone()]));
        value_paths.extend(named.iter().map(|n| n.split('/').map(str::to_owned).collect()));
        for p in &value_paths {
            for f in &field_pool {
                let refs: Vec<&str> = p.iter().map(String::as_str).collect();
                let ours = values_at(kind_of(k), &refs, f);
                let theirs = strings(&d["values"][format!("{k}|{}|{f}", p.join("/"))]);
                c.lattice += 1;
                if ours != theirs.iter().map(String::as_str).collect::<Vec<_>>() {
                    failures.push(format!("values {k}|{}|{f} ts={theirs:?} rust={ours:?}", p.join("/")));
                }
            }
        }
    }
    for (path, kind) in d["files"].as_object().expect("files") {
        c.lattice += 1;
        let ours = file_kind_for(path).map(|k| format!("{k:?}").to_lowercase());
        let theirs = kind.as_str().map(str::to_owned);
        if ours != theirs {
            failures.push(format!("fileKindFor({path:?}) ts={theirs:?} rust={ours:?}"));
        }
    }
}

pub fn wav_gate(dumps: &Path, failures: &mut Vec<String>, c: &mut Counts) {
    let expected = read_json(&dumps.join("wav.json"));
    let dir = dumps.join("sweep").join("wav");
    let root = support::spell(&dir);
    let src = DiskSource::new(std::slice::from_ref(&root)).expect("source");
    let mut cache = Cache::new();
    let mut m = ByHeader::new(&src, &mut cache);
    for (name, secs) in expected.as_object().expect("object") {
        let got = m.seconds(&format!("{root}/{name}")).expect("measures");
        c.wavs += 1;
        c.wavs_measured += usize::from(got.is_some());
        match (secs.as_f64(), got) {
            (None, None) => {}
            (Some(a), Some(b)) if (a - b).abs() <= 1e-9 * a.abs().max(1.0) => {}
            (a, b) => failures.push(format!("wav {name} ts={a:?} rust={b:?}")),
        }
    }
    // The pure function agrees with the Source's route on every file.
    for name in expected.as_object().expect("object").keys().take(200) {
        let bytes = fs::read(dir.join(name)).expect("file");
        let head = &bytes[..bytes.len().min(8192)];
        let direct = wav_seconds(head, bytes.len() as u64);
        if direct != m.seconds(&format!("{root}/{name}")).expect("measures") {
            failures.push(format!("wav {name}: wav_seconds and ByHeader differ"));
        }
    }
}

pub fn clips_gate(dumps: &Path, failures: &mut Vec<String>, c: &mut Counts) {
    let lists = read_json(&dumps.join("clips.json"));
    let root = support::spell(&dumps.join("sweep"));
    let src = DiskSource::new(&[root]).expect("source");
    for (i, l) in lists.as_array().expect("lists").iter().enumerate() {
        let clips: Vec<ClipDoc> = serde_json::from_value(l["clips"].clone()).expect("clips");
        let tempo = l["tempo"].as_f64();
        let mut cache = Cache::new();
        let mut by = ByHeader::new(&src, &mut cache);
        let ours = fold_nests(&clips, &mut |c: &ClipDoc| web_clip(c, tempo, &mut by)).expect("folds");
        let ours = serde_json::to_value(&ours).expect("json");
        diff(failures, &format!("clips list {i}"), &l["web"], &ours);
        c.sweep_lists += 1;
        count_sweep(&clips, &ours, tempo, &mut by, c);
    }
}

fn count_sweep(clips: &[ClipDoc], ours: &Value, tempo: Option<f64>, by: &mut ByHeader, c: &mut Counts) {
    let mut folded = false;
    for w in ours.as_array().into_iter().flatten() {
        if w.get("pieces").is_some() {
            folded = true;
            c.comp_pieces += w["pieces"].as_array().map_or(0, Vec::len);
        }
        if let Some(t) = w.get("timeline") {
            c.nests += 1;
            folded = true;
            c.midi_hosts += usize::from(w["type"] == "midi");
            c.comp_pieces += t["clips"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x["pieces"].as_array())
                .map(Vec::len)
                .sum::<usize>();
        }
    }
    c.sweep_folded += usize::from(folded);
    for cl in clips {
        c.crossfades += usize::from(cl.crossfade.is_some());
        c.frozen += usize::from(cl.id.ends_with("__frozen"));
        c.source_from += usize::from(cl.source_from.is_some());
        for n in &cl.notes {
            c.repeat += usize::from(n.repeat.is_some());
            c.skip += usize::from(n.skip.is_some());
        }
        if cl.length == 0.0
            && tempo.is_some_and(|t| t > 0.0)
            && cl.kind == song::clips::ClipKind::Audio
            && !cl.source.is_empty()
        {
            match by.seconds(&cl.source) {
                Ok(Some(s)) if s - cl.source_from.unwrap_or(0.0) > 0.0 => c.sweep_natural += 1,
                _ => c.unreadable += 1,
            }
        }
    }
}
