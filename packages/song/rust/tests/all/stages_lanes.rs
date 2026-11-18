// stages_lanes.rs — the stage gate of 2e: lanes, owners, live inputs, faults, assemblies and the console against the TypeScript's dumps.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps/lanes> cargo test --test all -- --ignored stages_lanes::
//
// What a reader gets wrong: the allow list is EMPTY, faults are compared as sets, and where the TypeScript threw at load the Rust must stop.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDumpLanes.ts.

use crate::lanes_support;
use crate::support;

use lanes_support::{Counts, Ran, check, lane_cases, lay, run};
use serde_json::{Value, json};
use song::assemblies::BUDGET;
use song::compare::{Rules, compare_all};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots};
use std::fs;
use std::path::{Path, PathBuf};

/// The faults only the wiring check raises: the TypeScript never reaches it when a reference was unresolved.
const WIRING: [&str; 4] = ["group_source|", "too_few|", "too_many|", "port_type|"];

struct Gate {
    cache: Cache,
    failures: Vec<String>,
    counts: Counts,
    projects: usize,
    threw: usize,
    negative: usize,
    negative_ts: usize,
    max_weight: usize,
    bundles: usize,
}

fn dump_of(dumps: &Path, name: &str) -> Value {
    let path = dumps.join(format!("{}.json", name.replace('/', "__")));
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("json")
}

impl Gate {
    /// One project read through a Recorder with the shared cache and again through the bundle it made.
    fn read(&mut self, name: &str, roots: &Roots, rig: &Value, mem: bool) -> Option<Ran> {
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = match run(&rec, &mut self.cache, roots, rig) {
            Ok(r) => r,
            Err(e) => {
                self.failures.push(format!("{name}: {e}"));
                return None;
            }
        };
        if mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            match MemSource::from_bundle(bundle).map(|m| run(&m, &mut Cache::new(), roots, rig)) {
                Ok(Ok(again))
                    if again.json == ran.json && again.stopped == ran.stopped && again.warned == ran.warned =>
                {
                    self.bundles += 1;
                }
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

    fn part(&mut self, name: &str, key: &str, theirs: &Value, ours: &Value) {
        let rules = Rules {
            max: 10,
            ..Rules::default()
        };
        let out = compare_all(theirs, ours, &rules);
        self.failures.extend(
            out.differences
                .iter()
                .map(|d| format!("{name} /{key}{} ts={:?} rust={:?}", d.pointer, d.left, d.right)),
        );
    }

    /// The TypeScript's dump against the Rust's, part by part.
    fn compare(&mut self, name: &str, ran: &Ran, theirs: &Value) {
        if let Some(why) = theirs["threw"].as_str() {
            self.threw += 1;
            if ran.stopped.is_empty() {
                self.failures.push(format!(
                    "{name}: the TypeScript threw at load ({}) and the Rust stops on nothing",
                    why.chars().take(80).collect::<String>()
                ));
            }
            return;
        }
        // A broken timeline stops the Rust at the read (249) and the TypeScript at its reference check, naming the same file.
        let broken = |f: &str| {
            theirs["faults"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|v| v.as_str() == Some(&format!("timeline_broken|file:{f}|")))
        };
        let unexplained: Vec<&String> = ran.stopped.iter().filter(|f| !broken(f)).collect();
        if !unexplained.is_empty() {
            self.failures.push(format!(
                "{name}: the Rust stops on {unexplained:?} and the TypeScript did not throw"
            ));
        }
        for key in [
            "lanes",
            "pictures",
            "video_tracks",
            "assemblies",
            "assembly_issues",
            "console",
        ] {
            self.part(name, key, &theirs[key], &ran.json[key]);
        }
        if theirs["own_threw"].is_null() {
            for key in ["owners", "bus", "input", "tap", "cued", "cue_pair", "side", "live"] {
                self.part(name, key, &theirs[key], &ran.json[key]);
            }
        }
        let mut ours = ran.faults.clone();
        if theirs["wiring_ran"] == json!(false) {
            ours.retain(|f| !WIRING.iter().any(|w| f.starts_with(w)));
        }
        let mut want: Vec<String> = theirs["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        want.sort();
        if ours != want {
            self.failures.push(format!("{name} /faults ts={want:?} rust={ours:?}"));
        }
    }
}

const MINIMUMS: [(&str, usize); 5] = [
    ("projects compared", 170),
    ("negative cases", 70),
    ("negative cases with the TypeScript", 55),
    ("bundle replays", 170),
    ("sound lanes", 350),
];

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn lanes_owners_inputs_and_checks_agree_with_the_typescript_over_the_corpus() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(staged.join("manifest.json")).expect("manifest")).expect("json");
    let mut gate = Gate {
        cache: Cache::new(),
        failures: Vec::new(),
        counts: Counts::default(),
        projects: 0,
        threw: 0,
        negative: 0,
        negative_ts: 0,
        max_weight: 0,
        bundles: 0,
    };
    let mut golden = Counts::default();
    for p in manifest["projects"].as_array().expect("projects") {
        let name = p["project"].as_str().expect("name");
        let theirs = dump_of(&dumps, name);
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("the dump's roots");
        if let Some(ran) = gate.read(name, &roots, &json!({}), true) {
            gate.compare(name, &ran, &theirs);
            golden.add(&ran.counts);
        }
        gate.projects += 1;
    }
    let mut negative = Counts::default();
    for case in lane_cases() {
        let base = staged.join("negative").join(format!("lanes-{}", case.name));
        lay(&base, &case);
        let roots = support::case_roots(&base, None);
        let Some(ran) = gate.read(&format!("negative-lanes/{}", case.name), &roots, &case.rig, case.mem) else {
            continue;
        };
        gate.failures.extend(check(&case, &ran));
        if case.ts {
            let theirs = dump_of(&dumps, &format!("negative-lanes/{}", case.name));
            gate.compare(&case.name, &ran, &theirs);
            gate.negative_ts += 1;
        }
        negative.add(&ran.counts);
        gate.negative += 1;
    }
    gate.counts.add(&golden);
    gate.counts.add(&negative);
    let (c, n) = (&golden, &negative);
    eprintln!(
        "lanes gate: {} projects compared, {} negative cases ({} with the TypeScript), {} TypeScript throws, {} bundle replays",
        gate.projects, gate.negative, gate.negative_ts, gate.threw, gate.bundles
    );
    eprintln!(
        "  golden: sound lanes {}, group {}, pattern {}, picture {}, frames {}, events {}, chains {} ({} owned apart), master {}, bus {}, \
         input {}, cued {}, side {} ({} through a step), live {} ({} loud), assemblies {} ({} plugs, {} params), systems {}, faulty projects {}",
        c.sound,
        c.group,
        c.pattern,
        c.picture,
        c.frames,
        c.events,
        c.chains,
        c.owned_apart,
        c.master,
        c.bus,
        c.input,
        c.cued,
        c.side_resolved,
        c.side_step,
        c.live,
        c.live_loud,
        c.assemblies,
        c.plugs,
        c.params,
        c.systems,
        c.faulty
    );
    eprintln!(
        "  negative: tap chains {}, console halves {}, live with channels {}, cue pairs applied {}, fault kinds {}, source kinds {}, \
         side {} ({} through a step), assemblies {}, faulty {}",
        n.tap,
        n.halves,
        n.live_channels,
        n.cue_applied,
        n.kinds.len(),
        n.sources.len(),
        n.side_resolved,
        n.side_step,
        n.assemblies,
        n.faulty
    );
    eprintln!("assemblies budget: at most {} of {BUDGET}", gate.max_weight);
    if gate.max_weight.saturating_mul(10) > BUDGET {
        gate.failures
            .push(format!("a pass spent {}: past a tenth of the budget", gate.max_weight));
    }
    let all = &gate.counts;
    let floors: Vec<(&str, usize, usize)> = vec![
        ("projects compared", gate.projects, MINIMUMS[0].1),
        ("negative cases", gate.negative, MINIMUMS[1].1),
        ("negative cases with the TypeScript", gate.negative_ts, MINIMUMS[2].1),
        ("bundle replays", gate.bundles, MINIMUMS[3].1),
        ("sound lanes", c.sound, MINIMUMS[4].1),
        ("group lanes", c.group, 4),
        ("pattern lanes", c.pattern, 6),
        ("picture lanes", c.picture, 8),
        ("picture lanes with a frame", c.frames, 3),
        ("events on lanes", c.events, 350),
        ("chains", c.chains, 500),
        (
            "chains owned by a bus, an input, the master or a tap",
            c.owned_apart,
            170,
        ),
        ("master chains", c.master, 170),
        ("bus chains", c.bus, 4),
        ("input chains", c.input, 1),
        ("cued chains", c.cued, 1),
        ("side inputs that resolved a track", c.side_resolved, 400),
        (
            "side inputs through a step in a run (negative corpus)",
            all.side_step,
            1,
        ),
        ("live inputs", c.live, 1),
        ("live inputs with a non-auto monitor", c.live_loud, 1),
        ("assemblies", c.assemblies, 1),
        ("plugs resolved", c.plugs, 2),
        ("params promoted", c.params, 8),
        ("console systems listed", c.systems, 1_000),
        ("projects with a fault", c.faulty, 1),
        ("tap chains (negative corpus)", n.tap, 1),
        ("console halves resolved (negative corpus)", n.halves, 1),
        ("live inputs with channels (negative corpus)", n.live_channels, 1),
        ("cue pairs applied (negative corpus)", n.cue_applied, 1),
        ("fault kinds (negative corpus)", n.kinds.len(), 20),
        ("wiring source kinds (negative corpus)", n.sources.len(), 10),
        ("TypeScript throws at load", gate.threw, 1),
    ];
    for (what, got, min) in floors {
        if got < min {
            gate.failures.push(format!(
                "the gate saw {got} {what}, fewer than {min}: it proves nothing"
            ));
        }
    }
    assert!(
        gate.failures.is_empty(),
        "{} differences:\n{}",
        gate.failures.len(),
        gate.failures.join("\n")
    );
}
