// stages_clips.rs — the stage gate of the placements: the Rust against the TypeScript's dumps over the staged corpus and the negative one.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps/clips> cargo test --test all -- --ignored stages_clips::
//
// What a reader gets wrong: it compares placements lane by lane, and a lane the TypeScript threw on must carry an ERROR on its file here.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDumpClips.ts.

use crate::clips_support;
use crate::support;

use clips_support::{Counts, check, clip_cases, lay, run};
use serde_json::{Value, json};
use song::compare::{Rules, compare_all};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots};
use std::fs;
use std::path::{Path, PathBuf};

const MIN_PROJECTS: usize = 170;
const MIN_CHAIN: usize = 1;
const MIN_LANES: usize = 340;
const MIN_EVENTS: usize = 340;
const MIN_AUDIO: usize = 250;
const MIN_SEQUENCES: usize = 70;
const MIN_PATTERN_LANES: usize = 3;
const MIN_NEGATIVE: usize = 55;
const MIN_NEGATIVE_TS: usize = 40;

struct Gate {
    cache: Cache,
    failures: Vec<String>,
    counts: Counts,
    max_weight: usize,
    projects: usize,
    chain: usize,
    negative: usize,
    negative_ts: usize,
}

fn dump_of(dumps: &Path, name: &str) -> Value {
    let path = dumps.join(format!("{}.json", name.replace('/', "__")));
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("json")
}

impl Gate {
    /// One project read through a Recorder with the shared cache and again through its bundle; None for a chain project.
    fn read(&mut self, name: &str, roots: &Roots, mem: bool) -> Option<clips_support::Ran> {
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = match run(&rec, &mut self.cache, roots) {
            Ok(r) => r,
            Err(e) => {
                self.failures.push(format!("{name}: {e}"));
                return None;
            }
        };
        if mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            match MemSource::from_bundle(bundle).map(|m| run(&m, &mut Cache::new(), roots)) {
                Ok(Ok(again)) if again.json == ran.json && again.refused == ran.refused => {}
                Ok(Ok(_)) => self
                    .failures
                    .push(format!("{name}: the bundle read differently from the disk")),
                Ok(Err(e)) => self.failures.push(format!("{name}: the bundle was not enough: {e}")),
                Err(e) => self.failures.push(format!("{name}: {e}")),
            }
        }
        self.max_weight = self.max_weight.max(ran.weight);
        Some(ran)
    }

    fn compare_one(&mut self, name: &str, at: &str, theirs: &Value, ours: &Value) {
        let rules = Rules {
            max: 10,
            ..Rules::default()
        };
        let out = compare_all(theirs, ours, &rules);
        self.failures.extend(
            out.differences
                .iter()
                .map(|d| format!("{name} {at}{} ts={:?} rust={:?}", d.pointer, d.left, d.right)),
        );
    }

    /// The TypeScript's dump against the Rust's, part by part; a lane it threw on must be an ERROR on its file here.
    fn compare(&mut self, name: &str, ran: &clips_support::Ran, theirs: &Value) {
        if theirs["chain"].as_bool().unwrap_or(false) != ran.chain {
            return self
                .failures
                .push(format!("{name}: the chain project is told differently"));
        }
        if ran.chain {
            return;
        }
        let threw = theirs["threw"].as_object().cloned().unwrap_or_default();
        for lane in threw.keys() {
            if !ran.refused.contains(&format!("nodes/{lane}.k7x")) {
                self.failures.push(format!(
                    "{name}: the TypeScript threw on lane {lane} and the Rust found no error on it"
                ));
            }
        }
        let lanes: serde_json::Map<String, Value> = ran.json["lanes"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, _)| !threw.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.compare_one(name, "/lanes", &theirs["lanes"], &Value::Object(lanes));
        if theirs["lengths_threw"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty)
        {
            self.compare_one(name, "/lengths", &theirs["lengths"], &ran.json["lengths"]);
        }
        if theirs["timelines_threw"].is_null() {
            self.compare_one(name, "/timelines", &theirs["timelines"], &ran.json["timelines"]);
        }
        for part in ["shares", "banks", "patterns"] {
            self.compare_one(name, &format!("/{part}"), &theirs[part], &ran.json[part]);
        }
        if threw.is_empty()
            && ran
                .refused
                .iter()
                .any(|f| f.starts_with("nodes/") || f.starts_with("timelines/"))
        {
            self.failures.push(format!(
                "{name}: the Rust refused {:?} and the TypeScript threw nowhere",
                ran.refused
            ));
        }
    }
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_placements_agree_with_the_typescript_over_the_corpus() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(staged.join("manifest.json")).expect("manifest")).expect("json");
    let mut gate = Gate {
        cache: Cache::new(),
        failures: Vec::new(),
        counts: Counts::default(),
        max_weight: 0,
        projects: 0,
        chain: 0,
        negative: 0,
        negative_ts: 0,
    };
    for p in manifest["projects"].as_array().expect("projects") {
        let name = p["project"].as_str().expect("name");
        let theirs = dump_of(&dumps, name);
        if theirs["chain"].as_bool().unwrap_or(false) {
            gate.chain += 1;
            continue;
        }
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("the dump's roots");
        if let Some(ran) = gate.read(name, &roots, true) {
            gate.compare(name, &ran, &theirs);
            gate.counts.add(&ran.counts);
        }
        gate.projects += 1;
    }
    // The negative corpus: the Rust's expectations always, the TypeScript's answers where it can have one.
    for case in clip_cases() {
        let base = staged.join("negative").join(format!("clips-{}", case.name));
        lay(&base, &case);
        let roots = support::case_roots(&base, None);
        let Some(ran) = gate.read(&format!("negative-clips/{}", case.name), &roots, case.mem) else {
            continue;
        };
        gate.failures.extend(check(&case, &ran));
        if case.ts {
            let theirs = dump_of(&dumps, &format!("negative-clips/{}", case.name));
            gate.compare(&case.name, &ran, &theirs);
            gate.negative_ts += 1;
        }
        gate.counts.add(&ran.counts);
        gate.negative += 1;
    }
    let c = gate.counts.clone();
    eprintln!(
        "placements gate: {} projects compared, {} chain projects skipped, {} lanes, {} events, {} audio clips, {} sequences, \
         {} pattern lanes, {} negative cases ({} with the TypeScript)",
        gate.projects,
        gate.chain,
        c.lanes,
        c.events,
        c.audio,
        c.sequences,
        c.pattern_lanes,
        gate.negative,
        gate.negative_ts
    );
    eprintln!(
        "  takes {}, comp pieces {}, crossfade marks {}, timeline docs {}, nested clips {}, seconds starts {}, fractional ids {}, \
         loop ranges {}, event windows {}, clip windows {}, gains {}, warps {}, warp_off {}, disabled {}, pad effects {}",
        c.takes,
        c.comp_pieces,
        c.crossfades,
        c.timeline_docs,
        c.nested,
        c.seconds_starts,
        c.fractional_ids,
        c.loop_ranges,
        c.event_windows,
        c.clip_windows,
        c.gains,
        c.warps,
        c.warp_offs,
        c.disabled,
        c.pad_effects
    );
    eprintln!("budget: at most {} of {}", gate.max_weight, song::clips::BUDGET);
    if gate.max_weight.saturating_mul(10) > song::clips::BUDGET {
        gate.failures
            .push(format!("a pass spent {}: past a tenth of the budget", gate.max_weight));
    }
    let mins = [
        ("projects", gate.projects, MIN_PROJECTS),
        ("chain projects skipped", gate.chain, MIN_CHAIN),
        ("lanes", c.lanes, MIN_LANES),
        ("events", c.events, MIN_EVENTS),
        ("audio clips", c.audio, MIN_AUDIO),
        ("sequences placed", c.sequences, MIN_SEQUENCES),
        ("pattern lanes", c.pattern_lanes, MIN_PATTERN_LANES),
        ("negative cases", gate.negative, MIN_NEGATIVE),
        ("negative cases with the TypeScript", gate.negative_ts, MIN_NEGATIVE_TS),
        ("takes", c.takes, 1),
        ("comp pieces", c.comp_pieces, 1),
        ("crossfade marks", c.crossfades, 1),
        ("timeline docs", c.timeline_docs, 1),
        ("nested clips", c.nested, 1),
        ("seconds starts", c.seconds_starts, 1),
        ("fractional ids", c.fractional_ids, 1),
        ("loop ranges", c.loop_ranges, 1),
        ("event windows", c.event_windows, 1),
        ("clip windows", c.clip_windows, 1),
        ("gains", c.gains, 1),
        ("warps", c.warps, 1),
        ("warp_off", c.warp_offs, 1),
        ("disabled", c.disabled, 1),
        ("pad effects", c.pad_effects, 1),
    ];
    for (what, got, min) in mins {
        if got < min {
            gate.failures.push(format!(
                "the gate saw {got} {what}, fewer than {min}: it proves nothing"
            ));
        }
    }
    let _ = json!(null);
    assert!(
        gate.failures.is_empty(),
        "{} differences:\n{}",
        gate.failures.len(),
        gate.failures.join("\n")
    );
}
