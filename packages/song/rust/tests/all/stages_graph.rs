// stages_graph.rs — the stage gate of 2k: the WebGraph and the graph model against the TypeScript's, over the corpus, the negative cases and the sweep.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps/graph> cargo test --test all -- --ignored stages_graph::
//
// What a reader gets wrong: the allow list and the strip list are EMPTY, and where the TypeScript threw the Rust must refuse or name the file.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDumpGraph.ts.

use crate::graph_support;
use crate::support;

use graph_support::{Counts, Ran, Via, check, graph_cases, lay, lay_renders, no_null, run};
use serde_json::{Value, json};
use song::compare::{Rules, compare_all};
use song::graph::BUDGET;
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots};
use std::fs;
use std::path::{Path, PathBuf};

struct Gate {
    cache: Cache,
    failures: Vec<String>,
    bundles: usize,
    threw: usize,
    max_spent: usize,
}

fn dump_of(dumps: &Path, name: &str) -> Value {
    let path = dumps.join(format!("{}.json", name.replace('/', "__")));
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("json")
}

/// The array at `key` sorted by its items' text: the sets the gate compares apart from order.
fn as_sets(v: &mut Value, keys: &[&str]) {
    for k in keys {
        if let Some(a) = v.get_mut(*k).and_then(Value::as_array_mut) {
            a.sort_by_key(ToString::to_string);
        }
    }
}

impl Gate {
    /// One project read through a Recorder with the shared cache and again through the bundle it made.
    fn read(&mut self, name: &str, roots: &Roots, rig: &Value, via: Via<'_>, mem: bool) -> Option<Ran> {
        let (mut mounts, files) = roots.mounts().expect("mounts");
        // The host mounts what a library links out to (the staged libraries link their sounds into the repository).
        for lib in &roots.libraries {
            for family in ["samples", "irs", "wavetables", "models"] {
                if let Ok(real) = fs::canonicalize(format!("{}/{family}", lib.root)) {
                    mounts.push(support::spell(&real));
                }
            }
        }
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = match run(&rec, &mut self.cache, roots, rig, via) {
            Ok(r) => r,
            Err(e) => {
                self.failures.push(format!("{name}: {e}"));
                return None;
            }
        };
        if mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            match MemSource::from_bundle(bundle).map(|m| run(&m, &mut Cache::new(), roots, rig, via)) {
                Ok(Ok(again)) if again.same_as(&ran) => {
                    self.bundles += 1;
                }
                Ok(Ok(_)) => self
                    .failures
                    .push(format!("{name}: the bundle read differently from the disk")),
                Ok(Err(e)) => self.failures.push(format!("{name}: the bundle was not enough: {e}")),
                Err(e) => self.failures.push(format!("{name}: {e}")),
            }
        }
        self.max_spent = self.max_spent.max(ran.spent);
        let mut bad = Vec::new();
        if !ran.refused {
            no_null(&ran.json, "", &mut bad);
        }
        self.failures.extend(bad.into_iter().map(|b| format!("{name}: {b}")));
        Some(ran)
    }

    fn part(&mut self, name: &str, key: &str, theirs: &Value, ours: &Value) {
        let rules = Rules {
            max: 8,
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
            let named = |f: &str| why.contains(f);
            let ok = if ran.refused {
                ran.refused_files.iter().any(|f| named(f)) || (why.contains("wiring") && !ran.refused_faults.is_empty())
            } else {
                ran.findings.iter().any(|(f, _)| named(f))
            };
            if !ok {
                self.failures.push(format!(
                    "{name}: the TypeScript threw ({}) and the Rust says nothing of it (refused {}, files {:?}, faults {:?}, findings {:?})",
                    why.lines().next().unwrap_or("").chars().take(100).collect::<String>(),
                    ran.refused,
                    ran.refused_files,
                    ran.refused_faults,
                    ran.finding_files()
                ));
            }
            return;
        }
        if ran.refused {
            self.failures.push(format!(
                "{name}: the Rust refuses ({:?} {:?}) and the TypeScript drew",
                ran.refused_files, ran.refused_faults
            ));
            return;
        }
        // The Handed the Rust made itself against the one the TypeScript's compile gave.
        if ran.real {
            self.part(name, "handed", &theirs["handed"], &ran.handed);
        }
        let (mut tg, mut og) = (theirs["graph"].clone(), ran.json.clone());
        as_sets(&mut tg, &["warnings"]);
        as_sets(&mut og, &["warnings"]);
        for v in [&mut tg, &mut og] {
            for k in ["tracks", "videoTracks"] {
                if let Some(a) = v.get_mut(k).and_then(Value::as_array_mut) {
                    a.sort_by_key(ToString::to_string);
                }
            }
            if let Some(a) = v.get_mut("issues").and_then(Value::as_array_mut) {
                a.sort_by_key(ToString::to_string);
            }
        }
        self.part(name, "graph", &tg, &og);
        let (mut tm, mut om) = (theirs["model"].clone(), ran.model.clone());
        as_sets(&mut tm, &["issues", "warnings", "disabledTracks"]);
        as_sets(&mut om, &["issues", "warnings", "disabledTracks"]);
        self.part(name, "model", &tm, &om);
    }
}

fn floor(gate: &mut Gate, what: &str, got: usize, min: usize) {
    if got < min {
        gate.failures.push(format!(
            "the gate saw {got} {what}, fewer than {min}: it proves nothing"
        ));
    }
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_graph_agrees_with_the_typescript_over_the_corpus_the_negative_cases_and_the_sweep() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(staged.join("manifest.json")).expect("manifest")).expect("json");
    let mut gate = Gate {
        cache: Cache::new(),
        failures: Vec::new(),
        bundles: 0,
        threw: 0,
        max_spent: 0,
    };
    let (mut golden, mut negative, mut sweep) = (Counts::default(), Counts::default(), Counts::default());
    let (mut projects, mut chain_projects) = (0usize, 0usize);
    for p in manifest["projects"].as_array().expect("projects") {
        let name = p["project"].as_str().expect("name");
        let theirs = dump_of(&dumps, name);
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("the dump's roots");
        if let Some(ran) = gate.read(name, &roots, &json!({}), Via::Compile, true) {
            gate.compare(name, &ran, &theirs);
            golden.of(&ran);
            if ran.json.get("stacks").is_some() {
                chain_projects += 1;
            }
        }
        projects += 1;
    }
    let (mut cases, mut cases_ts) = (0usize, 0usize);
    for case in graph_cases() {
        let base = staged.join("negative").join(format!("graph-{}", case.name));
        lay(&base, &case);
        let name = format!("negative-graph/{}", case.name);
        // A case the TypeScript answers is read with the roots its dump used, and drawn over the Rust's own compile.
        let theirs = case.ts.then(|| dump_of(&dumps, &name));
        let roots: Roots = match &theirs {
            Some(t) => serde_json::from_value(t["roots"].clone()).expect("the dump's roots"),
            None => case.roots(&base),
        };
        if case.renders.as_deref() == Some("ready") {
            lay_renders(&roots, &case.rig);
        }
        let Some(ran) = gate.read(&name, &roots, &case.rig, case.via(), case.mem) else {
            continue;
        };
        if let Some(theirs) = &theirs {
            gate.compare(&case.name, &ran, theirs);
            cases_ts += 1;
        } else {
            gate.failures.extend(check(&case, &ran));
        }
        negative.of(&ran);
        cases += 1;
    }
    let list: Vec<String> = fs::read_to_string(dumps.join("sweep.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    for (n, name) in list.iter().enumerate() {
        let theirs = dump_of(&dumps, name);
        let roots: Roots = serde_json::from_value(theirs["roots"].clone()).expect("the dump's roots");
        // The dump made the renders of every other project, each side at the names its own compile gives.
        if n % 2 == 0 {
            lay_renders(&roots, &json!({}));
        }
        if let Some(ran) = gate.read(name, &roots, &json!({}), Via::Compile, true) {
            gate.compare(name, &ran, &theirs);
            sweep.of(&ran);
        }
    }
    eprintln!(
        "graph gate: {projects} projects ({chain_projects} chain projects), {cases} negative cases ({cases_ts} with the TypeScript), {} sweep projects, {} TypeScript throws, {} bundle replays",
        list.len(),
        gate.threw,
        gate.bundles
    );
    for (what, c) in [("golden", &golden), ("negative", &negative), ("sweep", &sweep)] {
        eprintln!("  {what}: {:?}", c.0);
    }
    eprintln!("the most any graph spent: {} of {BUDGET}", gate.max_spent);
    if gate.max_spent.saturating_mul(10) > BUDGET {
        gate.failures
            .push(format!("a graph spent {}: past a tenth of the budget", gate.max_spent));
    }
    floor(&mut gate, "projects compared", projects, 170);
    floor(&mut gate, "chain projects", chain_projects, 4);
    floor(&mut gate, "negative cases", cases, 100);
    floor(&mut gate, "negative cases with the TypeScript", cases_ts, 88);
    floor(&mut gate, "sweep projects", list.len(), 1_500);
    let mut everywhere = Counts::default();
    for c in [&golden, &negative, &sweep] {
        everywhere.add(c);
    }
    for (what, key, min) in [
        ("chains", "chains", 500),
        ("steps", "steps", 350),
        ("outs", "outs", 500),
        ("outs to a port", "outs to a port", 3),
        ("chains with fromPort", "chains with fromPort", 1),
        ("single lanes", "track single", 330),
        ("group lanes", "track group", 4),
        ("strudel lanes", "track strudel", 6),
        ("bypassed lanes", "bypassed track", 10),
        ("audio clips", "clip audio", 250),
        ("midi clips", "clip midi", 70),
        ("clips with a compiled len", "clip with len", 330),
        ("steps with a script", "panel with a script", 120),
        ("steps with a vertical", "panel with a vertical", 150),
        ("steps with tabs", "panel with tabs", 40),
        ("stub steps", "stub true", 30),
        ("curves", "top curves", 3),
        ("midi signals", "signal midi", 70),
        ("audio signals", "signal audio", 250),
        ("colours", "top trackColors", 330),
    ] {
        floor(&mut gate, what, golden.get(key), min);
    }
    for (what, key, min) in [
        ("steps with uses", "step key uses", 350),
        ("steps with stage", "step key stage", 300),
        ("steps with a pluginId", "step key pluginId", 300),
        ("steps with events", "step key events", 80),
        ("steps with defaults", "step key defaults", 50),
        ("steps with engines", "step key engines", 200),
        ("steps with a panel", "step key ui", 320),
    ] {
        floor(&mut gate, what, golden.get(key), min);
    }
    // Every key of the field tables is met at least three times over the corpus, the negative cases and the sweep, a MIDI map once.
    for key in [
        "uses",
        "place",
        "stage",
        "category",
        "emits",
        "enabled",
        "missing",
        "inputs",
        "outputs",
        "events",
        "assets",
        "assetChoices",
        "base",
        "bases",
        "shader",
        "shaders",
        "notSupported",
        "unknownPlugin",
        "hasNativePluginUI",
        "strip",
        "modelDownload",
        "ampModel",
        "ui",
        "defaults",
        "pluginId",
        "alternatives",
        "alternative",
        "engines",
        "engine",
        "monitor",
        "channel",
        "camera",
        "device",
        "tap",
        "automations",
        "rack",
        "rackKit",
    ] {
        floor(
            &mut gate,
            &format!("steps with {key}"),
            everywhere.get(&format!("step key {key}")),
            3,
        );
    }
    floor(&mut gate, "steps with a MIDI map", everywhere.get("step key cc"), 1);
    // Every code a file can make, three times over the negative cases and the sweep but for the four the page names.
    for code in song::graph::WarningCode::ALL {
        let n = code.name();
        let got = negative.get(&format!("issue {n}")) + sweep.get(&format!("issue {n}"));
        let min = match n {
            "outNegativeGain" | "chainDeadEnd" => continue,
            "inputTrackGroup" | "inputTrackStrudel" | "assemblyMemberMissing" => 1,
            _ => 3,
        };
        floor(&mut gate, &format!("{n} warnings"), got, min);
    }
    // What the sweep's pool names is held by the sweep alone, apart from the negative corpus.
    for (what, key, min) in [
        ("strudel lanes", "track strudel", 200),
        ("assemblies", "top assemblies", 250),
        ("nodes in two assemblies", "issue nodeTwoAssemblies", 90),
        ("boxes named after a node", "issue assemblyNameTaken", 30),
        ("stacks", "top stacks", 80),
        ("steps with a place", "step key place", 450),
        ("positions", "top positions", 400),
        ("tap feeds", "top tapFeeds", 120),
        ("steps with assetChoices", "step key assetChoices", 2_000),
        ("steps with a shader", "step key shader", 90),
        ("steps with a base", "step key base", 90),
    ] {
        floor(&mut gate, &format!("sweep {what}"), sweep.get(key), min);
    }
    let bare = sweep.get("steps").saturating_sub(sweep.get("step key assetChoices"));
    floor(&mut gate, "sweep steps without assetChoices", bare, 500);
    floor(&mut gate, "sweep refusals", sweep.get("refused"), 20);
    floor(&mut gate, "negative refusals", negative.get("refused"), 3);
    floor(
        &mut gate,
        "frozen marks (negative corpus)",
        negative.get("top frozen"),
        1,
    );
    floor(
        &mut gate,
        "auto marks (negative corpus)",
        negative.get("top autoFrozen"),
        1,
    );
    // What `handed_of` made over the Rust's own compile and the TypeScript agreed with: the real path of Part B, counted apart from the given one.
    for (what, key, min) in [
        ("golden projects drawn over the Rust's compile", "real compiled", 170),
        (
            "golden projects with colours made by handed_of",
            "real handed colors",
            150,
        ),
        (
            "golden projects with extents made by handed_of",
            "real handed extents",
            150,
        ),
        ("golden projects with mixers made by handed_of", "real handed mixers", 2),
        ("golden clips whose len handed_of made", "real clips with len", 330),
    ] {
        floor(&mut gate, what, golden.get(key), min);
    }
    for (what, key, min) in [
        ("negative manual plans made by handed_of", "real handed manual plans", 8),
        ("negative frozen marks over a real render", "real top frozen", 12),
        (
            "negative automatic freezes that play their render",
            "real auto ready",
            1,
        ),
        ("negative automatic freezes the engine hosts", "real auto hosted", 1),
        (
            "negative automatic freezes waiting for a render",
            "real auto waiting",
            1,
        ),
        ("negative autoFrozen marks over a real render", "real top autoFrozen", 1),
        (
            "negative autoFreezeWaiting marks over a real render",
            "real top autoFreezeWaiting",
            1,
        ),
        ("negative projects that did not compile", "real uncompiled", 20),
    ] {
        floor(&mut gate, what, negative.get(key), min);
    }
    for (what, key, min) in [
        ("sweep projects drawn over the Rust's compile", "real compiled", 600),
        ("sweep projects with no compile", "real uncompiled", 500),
        (
            "sweep projects with colours made by handed_of",
            "real handed colors",
            500,
        ),
        (
            "sweep projects with extents made by handed_of",
            "real handed extents",
            400,
        ),
        ("sweep projects with mixers made by handed_of", "real handed mixers", 10),
        ("sweep clips whose len handed_of made", "real clips with len", 500),
        ("sweep automatic freezes waiting", "real auto waiting", 2),
        ("sweep automatic freezes that play their render", "real auto ready", 6),
        ("sweep manual plans made by handed_of", "real handed manual plans", 300),
        ("sweep frozen marks", "real top frozen", 500),
        ("sweep autoFrozen marks", "real top autoFrozen", 8),
    ] {
        floor(&mut gate, what, sweep.get(key), min);
    }
    let (bundles, threw) = (gate.bundles, gate.threw);
    floor(&mut gate, "bundle replays", bundles, 170);
    floor(&mut gate, "TypeScript throws", threw, 1);
    assert!(
        gate.failures.is_empty(),
        "{} differences:\n{}",
        gate.failures.len(),
        gate.failures.join("\n")
    );
}
