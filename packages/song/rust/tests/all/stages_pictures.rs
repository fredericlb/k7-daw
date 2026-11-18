// stages_pictures.rs — the stage gate of the pictures: the Rust against the TypeScript's dumps over the staged corpus, the negative cases and the sweep.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps/pictures> cargo test --test all -- --ignored stages_pictures::
//
// What a reader gets wrong: the allow list is EMPTY, faults are compared as sets, and where the TypeScript threw at load the Rust must stop.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDumpPictures.ts.

use crate::pictures_support;
use crate::support;

use pictures_support::{Counts, Ran, check_case, lay_case, pic_cases, run};
use serde_json::Value;
use song::compare::{Rules, compare_all};
use song::pictures::BUDGET;
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots};
use std::fs;
use std::path::{Path, PathBuf};

struct Gate {
    cache: Cache,
    failures: Vec<String>,
    threw: usize,
    bundles: usize,
    empty_equal: usize,
}

fn dump_of(dumps: &Path, name: &str) -> Value {
    let path = dumps.join(format!("{}.json", name.replace('/', "__")));
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("json")
}

fn empty(v: &Value) -> bool {
    v.as_object().is_none_or(serde_json::Map::is_empty)
}

impl Gate {
    /// One project read through a Recorder with the shared cache and again through the bundle it made.
    fn read(&mut self, name: &str, roots: &Roots, mem: bool) -> Option<Ran> {
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
                Ok(Ok(again))
                    if again.json == ran.json && again.refused == ran.refused && again.warned == ran.warned =>
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

    /// The TypeScript's dump against the Rust's, part by part; true when the answer was compared.
    fn compare(&mut self, name: &str, ran: &Ran, theirs: &Value) -> bool {
        if let Some(why) = theirs["threw"].as_str() {
            self.threw += 1;
            if ran.stopped.is_empty() {
                self.failures.push(format!(
                    "{name}: the TypeScript threw ({}) and the Rust stops on nothing",
                    why.chars().take(80).collect::<String>()
                ));
            }
            return false;
        }
        if !ran.stopped.is_empty() {
            self.failures.push(format!(
                "{name}: the Rust stops on {:?} and the TypeScript did not throw",
                ran.stopped
            ));
        }
        for key in ["video_sounds", "picture_sounds", "placed"] {
            self.part(name, key, &theirs[key], &ran.json[key]);
        }
        let mut want: Vec<String> = theirs["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        want.sort();
        if ran.faults != want {
            self.failures
                .push(format!("{name} /faults ts={want:?} rust={:?}", ran.faults));
        }
        true
    }
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn picture_sounds_and_placed_shots_agree_with_the_typescript_over_the_corpus() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(staged.join("manifest.json")).expect("manifest")).expect("json");
    let mut gate = Gate {
        cache: Cache::new(),
        failures: Vec::new(),
        threw: 0,
        bundles: 0,
        empty_equal: 0,
    };
    let (mut golden, mut negative, mut sweep) = (Counts::default(), Counts::default(), Counts::default());
    let mut projects = 0;
    for p in manifest["projects"].as_array().expect("projects") {
        let name = p["project"].as_str().expect("name");
        let theirs = dump_of(&dumps, name);
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("the dump's roots");
        if let Some(ran) = gate.read(name, &roots, true) {
            if gate.compare(name, &ran, &theirs)
                && ["video_sounds", "picture_sounds", "placed"]
                    .iter()
                    .all(|k| empty(&ran.json[*k]))
                && ran.faults.is_empty()
            {
                gate.empty_equal += 1;
            }
            golden.add(&ran.counts);
        }
        projects += 1;
    }
    let (mut cases, mut cases_ts) = (0, 0);
    for case in pic_cases() {
        let base = staged.join("negative").join(format!("pictures-{}", case.name));
        lay_case(&base, &case, support::placed);
        let roots = support::case_roots(&base, None);
        let Some(ran) = gate.read(&format!("negative-pictures/{}", case.name), &roots, case.mem) else {
            continue;
        };
        gate.failures.extend(check_case(&case, &ran));
        if case.ts {
            let theirs = dump_of(&dumps, &format!("negative-pictures/{}", case.name));
            gate.compare(&case.name, &ran, &theirs);
            cases_ts += 1;
        }
        negative.add(&ran.counts);
        cases += 1;
    }
    let names: Vec<String> = serde_json::from_str(&fs::read_to_string(dumps.join("sweep.json")).expect("sweep.json"))
        .expect("the sweep's names");
    let mut swept = 0;
    for n in &names {
        let base = staged.join("sweep").join(n);
        let roots = support::case_roots(&base, None);
        let Some(ran) = gate.read(&format!("sweep/{n}"), &roots, true) else {
            continue;
        };
        let theirs = dump_of(&dumps, &format!("sweep/{n}"));
        if gate.compare(n, &ran, &theirs) {
            swept += 1;
        }
        sweep.add(&ran.counts);
    }
    report(
        &mut gate,
        [&golden, &negative, &sweep],
        [projects, cases, cases_ts, swept],
    );
}

fn report(gate: &mut Gate, counts: [&Counts; 3], [projects, cases, cases_ts, swept]: [usize; 4]) {
    let [golden, negative, sweep] = counts;
    eprintln!(
        "pictures gate: {projects} projects ({} both empty and equal, {} TypeScript throws, {} bundle replays), {cases} negative cases \
         ({cases_ts} with the TypeScript), {swept} sweep projects compared",
        gate.empty_equal, gate.threw, gate.bundles
    );
    eprintln!("  staged: {golden:?}");
    eprintln!("  negative: {negative:?}");
    eprintln!("  sweep: {sweep:?}");
    let spent = golden.max_spent.max(negative.max_spent).max(sweep.max_spent);
    eprintln!("budget: at most {spent} of {BUDGET}");
    if spent.saturating_mul(10) > BUDGET {
        gate.failures
            .push(format!("a call spent {spent}: past a tenth of the budget"));
    }
    let mut both = negative.clone();
    both.add(sweep);
    let floors: Vec<(&str, usize, usize)> = vec![
        ("staged: projects compared, both empty and equal", gate.empty_equal, 170),
        ("staged: picture lanes", golden.pic_lanes, 8),
        ("staged: picture lanes with a frame", golden.frames, 3),
        ("staged: events on picture lanes", golden.events, 6),
        ("staged: activations", golden.activations, 2),
        ("sweep projects compared", swept, 500),
        ("negative cases", cases, 60),
        ("negative cases with the TypeScript", cases_ts, 55),
        ("pieces", both.pieces, 4000),
        ("shots that re-entered after a cover", both.reentered, 250),
        ("shots cut by more than one later shot", both.multi_cut, 25),
        ("crossfades taken", both.crossfades, 600),
        ("crossfades where the last in file won", both.cross_ties, 150),
        ("open-ended pieces", both.open_pieces, 100),
        ("pieces with a sourceFrom", both.from_pieces, 1400),
        ("varispeed warps", both.varispeed, 400),
        ("elastique warps", both.elastique, 120),
        ("silent shots covering", both.silent_cover, 550),
        ("freeze shots covering", both.freeze_cover, 200),
        ("reverse shots covering", both.reverse_cover, 220),
        ("lanes sounded through video_source", both.video_lanes, 300),
        ("picture lanes sounded by their own half", both.own_lanes, 400),
        ("wired lanes with no clips", both.wired_empty, 25),
        ("video_source_both faults", both.fault_both, 40),
        ("video_source_absent faults", both.fault_absent, 30),
        ("sound_missing faults", both.fault_missing, 40),
        ("placed shots", both.placed, 1800),
        ("placed shots on a nest", both.nested, 400),
        ("placed shots cut at a timeline's end", both.cut, 450),
        ("placed shots given a length by the rule", both.given, 650),
        ("placed shots merged before a lane's own", both.merged, 300),
        ("placed shots that then sounded", both.sounded, 180),
    ];
    eprintln!(
        "kinds the staged corpus lacks, counted over the negative cases and the sweep: staged pieces {}, placed shots {}, faults {}",
        golden.pieces,
        golden.placed,
        golden.fault_both + golden.fault_absent + golden.fault_missing
    );
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
