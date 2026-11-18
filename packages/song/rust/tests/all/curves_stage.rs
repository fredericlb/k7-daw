// curves_stage.rs — the curves gate: the Rust curves against the TypeScript's over the corpus, the negative cases, a sweep and the wired lanes.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps> cargo test --test all -- --ignored curves_stage::
//
// What a reader gets wrong: an allowed difference names its decision and ONE case; the sweep's beats are compared within 1e-9, its values 1.5e-6.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/curveDump.ts.

#[path = "curves_stage/cover.rs"]
mod cover;
use crate::curves_support;
use crate::support;
#[path = "curves_stage/wired.rs"]
mod wired;

use cover::{Case, Counts, tally, without_cut};
use curves_support::sig;
use serde_json::{Map, Value, json};
use song::compare::{Allow, Rules, compare_all};
use song::curves::{
    BUDGET, Budget, CurveBaker, CurveFile, CurvesRead, ResolvedCurve, bake_adsr, bake_curve, curve_runs,
    envelope_fires, read_curves,
};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use std::fs;
use std::path::{Path, PathBuf};

/// The differences the gate lets pass: the case, the pointer, the decision that allows it.
fn allowed() -> Vec<(&'static str, Allow)> {
    Vec::new()
}

// Set below what the first green run saw (docs/rustCompilerCurves.md), and never zero.
const MIN_GOLDEN: usize = 4;
const MIN_NEGATIVE: usize = 30;
const MIN_NEGATIVE_TS: usize = 20;
const MIN_SWEPT: usize = 3000;
const MIN_EACH: usize = 50;
const MIN_WIRED: usize = 150;

struct Gate {
    allow: Vec<(&'static str, Allow)>,
    failures: Vec<String>,
    set_aside: Vec<Value>,
    n: Counts,
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("json")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).expect("json")
}

/// What a read answers, in the shape curveDump.ts writes: the curves by id and the files with findings.
fn ours_of(read: &CurvesRead) -> (Value, Vec<String>) {
    let curves: Map<String, Value> = read.curves.iter().map(|(id, c)| (id.clone(), json_of(&**c))).collect();
    let mut files: Vec<String> = read.findings.iter().map(|f| f.file.clone()).collect();
    files.sort();
    (Value::Object(curves), files)
}

impl Gate {
    fn compare(&mut self, name: &str, at: &str, theirs: &Value, ours: &Value, rules: Rules) {
        let mut rules = rules;
        rules.allow = self
            .allow
            .iter()
            .filter(|(p, _)| *p == name)
            .map(|(_, a)| a.clone())
            .collect();
        let out = compare_all(theirs, ours, &rules);
        self.failures.extend(
            out.differences
                .iter()
                .map(|d| format!("{name} {at}{} ts={:?} rust={:?}", d.pointer, d.left, d.right)),
        );
        self.set_aside.extend(
            out.allowed
                .iter()
                .map(|(d, why)| json!({"case": name, "pointer": format!("{at}{}", d.pointer), "reason": why})),
        );
    }

    /// One folder's Rust read against the TypeScript's dump; where the TypeScript threw, the Rust must have a finding.
    fn folder(&mut self, name: &str, ours: &CurvesRead, theirs: &Value) -> bool {
        let (curves, files) = ours_of(ours);
        if theirs["threw"].is_string() {
            self.n.skipped_threw += 1;
            if ours.findings.is_empty() {
                self.failures
                    .push(format!("{name}: the TypeScript threw and the Rust found nothing"));
            }
            return false;
        }
        let rules = Rules {
            max: 10,
            ..Rules::default()
        };
        self.compare(name, "/curves", &theirs["curves"], &curves, rules.clone());
        let mut want: Vec<String> = theirs["files"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        want.sort();
        if want != files {
            self.failures.push(format!(
                "{name}: files with findings {files:?}, the TypeScript's {want:?}"
            ));
        }
        true
    }

    /// Decision 218: beats within 1e-9, values within 1.5e-6, counts exact.
    fn baked(&mut self, name: &str, theirs: &Value, ours: &song::curves::BakedCurve) {
        let pts = theirs["points"].as_array().cloned().unwrap_or_default();
        if theirs["bipolar"].as_bool() != Some(ours.bipolar) {
            self.failures.push(format!("{name}: bipolar differs"));
        }
        if pts.len() != ours.points.len() {
            self.failures.push(format!(
                "{name}: {} points, the TypeScript's {}",
                ours.points.len(),
                pts.len()
            ));
            return;
        }
        for (i, (t, o)) in pts.iter().zip(&ours.points).enumerate() {
            let (tb, tv) = (t[0].as_f64().unwrap_or(f64::NAN), t[1].as_f64().unwrap_or(f64::NAN));
            let near = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()));
            if !near(tb, o[0], 1e-9) || (tv - o[1]).abs() > 1.5e-6 {
                self.failures
                    .push(format!("{name} point {i}: rust {:?}, ts [{tb}, {tv}]", o));
                return;
            }
        }
    }
}

fn staged_dirs() -> (PathBuf, PathBuf) {
    (
        PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED")),
        PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS")),
    )
}

/// Reads a folder through a Recorder, then again through the bundle it made with a cache of its own; both must agree.
fn read_twice(gate: &mut Gate, name: &str, project: &str, cache: &mut Cache) -> CurvesRead {
    let disk = DiskSource::new(&[project]).expect("source");
    let rec = Recorder::new(&disk);
    let ours = read_curves(&rec, cache, project).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mem = MemSource::from_bundle(rec.into_bundle(&[project.to_owned()])).expect("a bundle");
    match read_curves(&mem, &mut Cache::new(), project) {
        Ok(again) if ours_of(&again) == ours_of(&ours) => {}
        Ok(_) => gate
            .failures
            .push(format!("{name}: the bundle read differently from the disk")),
        Err(e) => gate.failures.push(format!("{name}: the bundle was not enough: {e}")),
    }
    ours
}

fn corpus(gate: &mut Gate, staged: &Path, dumps: &Path) {
    let manifest = read_json(&staged.join("manifest.json"));
    let theirs = read_json(&dumps.join("curves/corpus.json"));
    let mut cache = Cache::new();
    for p in manifest["projects"].as_array().expect("projects") {
        let (abs, name) = (p["abs"].as_str().expect("abs"), p["project"].as_str().expect("name"));
        let abs = support::spell(Path::new(abs));
        let ours = read_twice(gate, name, &abs, &mut cache);
        let dump = theirs
            .as_array()
            .and_then(|a| a.iter().find(|d| d["project"] == name))
            .unwrap_or_else(|| panic!("{name}: no TypeScript dump"));
        if gate.folder(name, &ours, dump) {
            gate.n.golden += ours.curves.len();
        }
        bake_golden(gate, &ours);
    }
}

/// Bakes each curve of a project as a song would, to see what the budget costs.
fn bake_golden(gate: &mut Gate, read: &CurvesRead) {
    let mut baker = CurveBaker::new(&read.curves, sig(4, 4), 120.0, 64.0);
    for id in read.curves.keys() {
        let _ = baker.bake(id, &[0.0, 4.0, 8.0]);
    }
    let (findings, spent) = baker.finish();
    if !findings.is_empty() {
        gate.failures
            .push(format!("a golden project's curves gave {findings:?}"));
    }
    gate.n.spent_max = gate.n.spent_max.max(spent);
}

fn negative(gate: &mut Gate, staged: &Path, dumps: &Path) {
    let all: Vec<Value> = serde_json::from_str(include_str!("negative_curves.json")).expect("negative_curves.json");
    gate.n.negative = all.len();
    for c in all.iter().filter(|c| c["ts"] == true) {
        let name = c["name"].as_str().expect("name");
        let project = support::spell(&staged.join("curves-negative").join(name));
        let theirs = read_json(&dumps.join(format!("curves/negative/{name}.json")));
        let ours = read_twice(gate, &format!("negative/{name}"), &project, &mut Cache::new());
        gate.folder(&format!("negative/{name}"), &ours, &theirs);
        gate.n.negative_ts += 1;
    }
}

fn runs_json(runs: &[song::curves::CurveRun]) -> Value {
    json!(
        runs.iter()
            .map(|r| json!({"from": r.from, "to": r.to, "origin": r.origin}))
            .collect::<Vec<_>>()
    )
}

fn fires_json(fires: &[song::curves::EnvelopeFire]) -> Value {
    json!(
        fires
            .iter()
            .map(|f| match f.stop {
                Some(s) => json!({"at": f.at, "stop": s, "retrigger": f.retrigger}),
                None => json!({"at": f.at, "retrigger": f.retrigger}),
            })
            .collect::<Vec<_>>()
    )
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn sweep(gate: &mut Gate, dumps: &Path) {
    let dump = read_json(&dumps.join("curves/sweep.json"));
    println!("sweep seed {}", dump["seed"]);
    let project = support::spell(&dumps.join("curves/sweep"));
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("source");
    let read = read_curves(&disk, &mut Cache::new(), &project).expect("the sweep reads");
    if !read.findings.is_empty() {
        gate.failures.push(format!(
            "the sweep gave findings: {:?}",
            &read.findings[..read.findings.len().min(3)]
        ));
    }
    for e in dump["entries"].as_array().expect("entries") {
        let id = e["id"].as_str().expect("id");
        let Some(file) = read.curves.get(id) else {
            gate.failures.push(format!("sweep {id}: the Rust did not read it"));
            continue;
        };
        let (num, den) = (e["sig"][0].as_u64().expect("num"), e["sig"][1].as_u64().expect("den"));
        let sg = sig(u32::try_from(num).expect("num"), u32::try_from(den).expect("den"));
        let (tempo, song) = (
            e["tempo"].as_f64().expect("tempo"),
            e["song_beats"].as_f64().expect("song"),
        );
        let triggers = floats(&e["triggers"]);
        let rc = match ResolvedCurve::new(file, sg) {
            Ok(rc) => rc,
            Err(err) => {
                gate.failures.push(format!("sweep {id}: does not resolve: {err}"));
                continue;
            }
        };
        let mut budget = Budget::new(BUDGET);
        gate.compare(
            &format!("sweep/{id}"),
            "/spec",
            &e["spec"],
            &json_of(&**file),
            Rules::default(),
        );
        let (baked, runs, fires) = bake_of(&rc, sg, tempo, song, &triggers, &mut budget);
        let (Ok(b), Ok(runs), Ok(fires)) = (baked, runs, fires) else {
            gate.failures.push(format!("sweep {id}: past the budget"));
            continue;
        };
        let name = format!("sweep/{id}");
        if rc.is_envelope() {
            gate.compare(&name, "/fires", &e["fires"], &fires_json(&fires), Rules::default());
        } else {
            gate.compare(&name, "/runs", &e["runs"], &runs_json(&runs), Rules::default());
        }
        gate.baked(&name, &e["baked"], &b);
        // The same curve with its cut removed, so a cut counts only where it changed a point.
        let bare = without_cut(file);
        let uncut = match ResolvedCurve::new(&bare, sg) {
            Ok(u) => bake_of(&u, sg, tempo, song, &triggers, &mut Budget::new(BUDGET))
                .0
                .unwrap_or_else(|_| b.clone()),
            Err(_) => b.clone(),
        };
        let case = Case {
            file,
            rc: &rc,
            sig: sg,
            tempo,
            song,
            triggers: &triggers,
            runs: &runs,
            fires: &fires,
            baked: &b,
            uncut: &uncut,
        };
        tally(&mut gate.n, &case);
        gate.n.spent_max = gate.n.spent_max.max(budget.spent());
        gate.n.swept += 1;
    }
}

type Bake<T> = Result<T, song::curves::Over>;

/// One curve's runs or fires and its bake, as the lane builder does them.
fn bake_of(
    rc: &ResolvedCurve,
    sg: song_schema::TimeSig,
    tempo: f64,
    song: f64,
    triggers: &[f64],
    budget: &mut Budget,
) -> (
    Bake<song::curves::BakedCurve>,
    Bake<Vec<song::curves::CurveRun>>,
    Bake<Vec<song::curves::EnvelopeFire>>,
) {
    if rc.is_envelope() {
        let fires = envelope_fires(rc, triggers, budget);
        let baked = fires.clone().and_then(|f| bake_adsr(rc, &f, tempo, budget));
        (baked, Ok(Vec::new()), fires)
    } else {
        let runs = curve_runs(rc, song.max(sg.bar() * 4.0), triggers, budget);
        (bake_curve(rc, song, triggers, budget), runs, Ok(Vec::new()))
    }
}

/// An hour of sixteenths on one lane with two cut envelopes: what one lane of a real song weighs.
fn full_size_lane(gate: &mut Gate) {
    let env = |attack: u32| {
        format!("type(\"adsr\")\nattack({attack})\ndecay(50)\nsustain(50)\nrelease(50)\ncut(\"positive\")\n")
    };
    let curves: indexmap::IndexMap<String, std::sync::Arc<CurveFile>> = [("a", 50), ("b", 60)]
        .iter()
        .map(|(id, attack)| {
            let file = curves_support::file(&env(*attack));
            ((*id).to_owned(), std::sync::Arc::new(file))
        })
        .collect();
    let notes: Vec<f64> = (0..28_800).map(|i| f64::from(i) / 4.0).collect();
    let mut baker = CurveBaker::new(&curves, sig(4, 4), 120.0, 7200.0);
    let window = song::curves::MidiWindow {
        start: 0.0,
        length: 0.0,
        loop_start: 0.0,
        loop_end: 0.0,
        note_times: &notes,
    };
    let events = baker.note_ons(&[window]);
    for id in ["a", "b"] {
        if baker.bake(id, &events).is_none_or(|c| c.points.len() < 100_000) {
            gate.failures
                .push(format!("the full-size lane's envelope {id} baked no lane"));
        }
    }
    let (findings, spent) = baker.finish();
    if !findings.is_empty() {
        gate.failures.push(format!("the full-size lane gave {findings:?}"));
    }
    gate.n.spent_lane = spent;
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_curves_agree_with_the_typescript_over_the_corpus_the_sweep_and_the_wired_lanes() {
    let (staged, dumps) = staged_dirs();
    let mut gate = Gate {
        allow: allowed(),
        failures: Vec::new(),
        set_aside: Vec::new(),
        n: Counts::default(),
    };
    corpus(&mut gate, &staged, &dumps);
    negative(&mut gate, &staged, &dumps);
    sweep(&mut gate, &dumps);
    full_size_lane(&mut gate);
    wired::wired(&mut gate, &dumps);
    let n = &gate.n;
    println!(
        "golden {} (the TypeScript threw on {}), negative {} ({} with the TypeScript), swept {}, wired lanes {} of {} bindings",
        n.golden, n.skipped_threw, n.negative, n.negative_ts, n.swept, n.wired, n.wired_bindings
    );
    println!(
        "kinds lfo/custom/onoff/adsr {:?}, conditions start/end/between/event {:?}, retrigger {}, duration {}, cut +{} -{}, many runs {}, envelopes cut short {}",
        n.kinds, n.conditions, n.retrigger, n.duration, n.cut_positive, n.cut_negative, n.many_runs, n.cut_short
    );
    println!(
        "coverage: grid widened {}, a position below 1 {}, four fields {}, deeper {}, fractional tempo {}, off-grid triggers {}, song of 1e5 or more {}, signature 1/64 or finer {}",
        n.widened, n.below_one, n.four_fields, n.deep_fields, n.fractional_tempo, n.off_grid, n.huge_song, n.fine_sig
    );
    println!(
        "budget: the most any sweep bake spent was {} of {BUDGET}, a full-size lane {}",
        n.spent_max, n.spent_lane
    );
    println!("{}", json!({"allowed": gate.set_aside}));
    let mut floors: Vec<(&str, usize, usize)> = vec![
        ("golden curves", n.golden, MIN_GOLDEN),
        ("negative cases", n.negative, MIN_NEGATIVE),
        ("negative cases with the TypeScript", n.negative_ts, MIN_NEGATIVE_TS),
        ("swept curves", n.swept, MIN_SWEPT),
        ("wired lanes", n.wired, MIN_WIRED),
        ("retrigger", n.retrigger, MIN_EACH),
        ("duration", n.duration, MIN_EACH),
        ("cut positive", n.cut_positive, MIN_EACH),
        ("cut negative", n.cut_negative, MIN_EACH),
        ("more than one run", n.many_runs, MIN_EACH),
        ("an envelope cut short", n.cut_short, MIN_EACH),
        ("a grid widened to its cap", n.widened, MIN_EACH),
        ("a position below 1", n.below_one, MIN_EACH),
        ("a position of four fields", n.four_fields, MIN_EACH),
        ("a position deeper than four fields", n.deep_fields, MIN_EACH),
        ("a fractional tempo", n.fractional_tempo, MIN_EACH),
        ("triggers off the eighth grid", n.off_grid, MIN_EACH),
        ("a song of 1e5 beats or more", n.huge_song, MIN_EACH),
        ("a signature of 1/64 or finer", n.fine_sig, MIN_EACH),
    ];
    floors.extend(
        ["lfo", "custom", "onoff", "adsr"]
            .iter()
            .zip(n.kinds)
            .map(|(k, c)| (*k, c, MIN_EACH)),
    );
    floors.extend(
        ["start", "end", "between", "event"]
            .iter()
            .zip(n.conditions)
            .map(|(k, c)| (*k, c, MIN_EACH)),
    );
    for (what, got, min) in floors {
        if got < min {
            gate.failures
                .push(format!("{what}: {got} seen, at least {min} expected"));
        }
    }
    for (what, spent) in [
        ("a sweep bake", gate.n.spent_max),
        ("a full-size lane", gate.n.spent_lane),
    ] {
        if spent == 0 || spent * 10 > BUDGET {
            gate.failures.push(format!(
                "{what} spent {spent}, which is not between 0 and a tenth of {BUDGET}"
            ));
        }
    }
    assert!(
        gate.failures.is_empty(),
        "{} differences:\n{}",
        gate.failures.len(),
        gate.failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
    );
}
