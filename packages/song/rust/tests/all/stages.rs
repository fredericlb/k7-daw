// stages.rs — the stage gate: the Rust readers against the TypeScript's dumps over the staged corpus and the negative one.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps> cargo test --test all -- --ignored stages::
//
// What a reader gets wrong: it compares readers, not a compile, and findings by FILE alone since the messages are rewritten.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDump.ts.

#[path = "stages/modules.rs"]
mod modules;
#[path = "stages/routing.rs"]
mod routing;
use crate::support;

use serde_json::{Map, Value, json};
use song::compare::{Allow, Rules, compare_all};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use support::{Case, ModCounts, cases, dump, finding_files};

/// The differences the gate lets pass, each for one project and with its reason; a pattern is relative to the family or the head.
fn allowed() -> Vec<(&'static str, Allow)> {
    let dead = |pattern: &str| {
        (
            "negative/dead-keys",
            Allow {
                pattern: pattern.to_owned(),
                reason: "project.k7x's dead `type` key is refused here and accepted by the TypeScript (docs/rustCompiler.md, decision 5)"
                    .to_owned(),
            },
        )
    };
    let dots = |pattern: &str| {
        (
            "negative/mod-dot",
            Allow {
                pattern: pattern.to_owned(),
                reason: "a dot folder is skipped by the walk and read by the TypeScript's tree (docs/rustCompiler.md, decision 20)"
                    .to_owned(),
            },
        )
    };
    let project_ref = |pattern: &str| {
        (
            "negative/res-project-sample",
            Allow {
                pattern: pattern.to_owned(),
                reason: "the vocabulary keeps an `@project/` sample unresolved: it is resolved per compile (docs/rustCompiler.md, decision 66)"
                    .to_owned(),
            },
        )
    };
    vec![
        project_ref("/@k7-core~1instruments~1kit/rack/symbols/BD/clip"),
        dead("/root_ok"),
        dead("/root/type"),
        dots("/@k7-core~1audio~1.hid"),
        dots("/@k7-core~1audio~1Cat~1.old"),
    ]
}

/// What a zero would mean: the gate ran over too little to say anything.
const MIN_PROJECTS: usize = 170;
const MIN_CLIPS: usize = 100;
const MIN_SEQUENCES: usize = 50;
const MIN_MARKERS: usize = 1;
const MIN_FINDING_FILES: usize = 1;
const MIN_NEGATIVE: usize = 8;
const MIN_MODULE_NEGATIVE: usize = 15;
// Set below what the first green run saw (docs/rustCompiler.md, "The stage gate"), and never zero.
const MIN_NODES: usize = 700;
const MIN_LIBRARY_MODULES: usize = 60;
const MIN_CATEGORY_MODULES: usize = 70;
const MIN_ENGINES: usize = 140;
const MIN_LIBRARY_NODES: usize = 1;
const MIN_LIBRARY_FINDINGS: usize = 4;
const MIN_SCAN_PLUGINS: usize = 1;
const MIN_VOCABULARY_HITS: usize = 100;
const MIN_RESOLVE_NEGATIVE: usize = 12;
// Resolution, set below the first green run (docs/rustCompiler.md): a kind with none in the golden corpus is counted over the negative one.
const MIN_PANELS: usize = 200;
const MIN_SCRIPTS: usize = 200;
const MIN_MAPS: usize = 1;
const MIN_PRESETS: usize = 5;
const MIN_RESOLVED_CLIPS: usize = 3;
const MIN_FILES: usize = 1;
const MIN_PROJECT_SAMPLES: usize = 1;
const MIN_FOLDS: usize = 800;
const MIN_ALTERNATIVES: usize = 1;
const MIN_ENGINES_RUN: usize = 200;
// Routing, set below the first green run (docs/rustCompiler.md, decision 105 and the 2d gate).
const MIN_CHAIN_PROJECTS: usize = 5;
const MIN_CHAIN_FILES: usize = 20;
const MIN_CHAIN_NODES: usize = 25;
const MIN_GRAPH_SEGMENTS: usize = 170;
const MIN_SUBCHAIN_NODES: usize = 1;
const MIN_PICTURE_LANES: usize = 4;
const MIN_LEVELS: usize = 1;
const MIN_BYPASSED: usize = 8;

/// A module of the vocabulary told apart as the minimums count them: each by its ref and the folder it was read from.
#[derive(Default)]
struct Seen {
    library: HashSet<String>,
    category: HashSet<String>,
    engines: HashSet<String>,
    nodes: HashSet<String>,
}

#[derive(Default)]
struct Totals {
    projects: usize,
    clips: usize,
    sequences: usize,
    timelines: usize,
    markers: usize,
    finding_files: usize,
    threw: usize,
    negative: usize,
    negative_timeline_findings: usize,
    remembered: usize,
    module_projects: usize,
    module_remembered: usize,
    module_unbundled: usize,
    linked_out: usize,
    linked_seen: HashSet<String>,
    module_negative: usize,
    resolve_negative: usize,
    mods: ModCounts,
    res: modules::ResCounts,
    route: routing::RouteCounts,
    seen: Seen,
}

struct Gate {
    cache: Cache,
    allow: Vec<(&'static str, Allow)>,
    totals: Totals,
    failures: Vec<String>,
    findings: Map<String, Value>,
    set_aside: Vec<Value>,
}

impl Gate {
    /// Compares one project's Rust dump with the TypeScript's, family by family and the head.
    fn compare(&mut self, name: &str, ours: &Value, theirs: &Value) -> bool {
        let mut threw = false;
        for sub in ["clips", "sequences", "timelines"] {
            let (o, t) = (&ours["families"][sub], &theirs["families"][sub]);
            if t["threw"].is_string() {
                // The TypeScript threw on a file the Rust names as a finding: both refuse the family.
                self.totals.threw += 1;
                threw = true;
                if o["files"].as_array().is_none_or(Vec::is_empty) {
                    self.failures
                        .push(format!("{name} {sub}: the TypeScript threw and the Rust found nothing"));
                }
                continue;
            }
            let theirs = json!({"values": t["values"], "files": t["files"]});
            self.record(name, &format!("/{sub}"), &theirs, o);
        }
        // The head is compared whole, but a family that threw leaves the TypeScript's root as it was.
        self.record(name, "/head", &theirs["head"], &ours["head"]);
        threw
    }

    fn record(&mut self, name: &str, at: &str, theirs: &Value, ours: &Value) {
        let allow = self
            .allow
            .iter()
            .filter(|(p, _)| *p == name)
            .map(|(_, a)| a.clone())
            .collect();
        let rules = Rules {
            allow,
            max: 10,
            ..Rules::default()
        };
        let out = compare_all(theirs, ours, &rules);
        self.failures.extend(
            out.differences
                .iter()
                .map(|d| format!("{name} {at}{} ts={:?} rust={:?}", d.pointer, d.left, d.right)),
        );
        self.set_aside.extend(
            out.allowed
                .iter()
                .map(|(d, why)| json!({"project": name, "pointer": format!("{at}{}", d.pointer), "reason": why})),
        );
    }

    /// The Rust dump of a project read through a Recorder, and the same read again through the bundle it made.
    fn read(&mut self, name: &str, project: &str, remember: bool) -> Value {
        let disk = DiskSource::new(&[project]).expect("source");
        let rec = Recorder::new(&disk);
        let ours = dump(&rec, project).unwrap_or_else(|e| panic!("{name}: {e}"));
        if remember {
            let bundle = rec.into_bundle(&[project.to_owned()]);
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            match dump(&mem, project) {
                Ok(again) if again.json == ours.json && again.compile_findings == ours.compile_findings => {
                    self.totals.remembered += 1;
                }
                Ok(_) => self
                    .failures
                    .push(format!("{name}: the bundle read differently from the disk")),
                Err(e) => self.failures.push(format!("{name}: the bundle was not enough: {e}")),
            }
        }
        ours.json
    }

    fn count(&mut self, ours: &Value) {
        let n = |sub: &str| ours["families"][sub]["values"].as_object().map_or(0, Map::len);
        self.totals.clips += n("clips");
        self.totals.sequences += n("sequences");
        self.totals.timelines += n("timelines");
        self.totals.markers += ours["head"]["markers"].as_array().map_or(0, Vec::len);
        for sub in ["clips", "sequences", "timelines"] {
            self.totals.finding_files += finding_files(ours, sub).len();
        }
    }
}

fn dump_of(dumps: &std::path::Path, name: &str) -> Value {
    let path = dumps.join(format!("{}.json", name.replace('/', "__")));
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("json")
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_readers_agree_with_the_typescript_over_the_corpus() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(staged.join("manifest.json")).expect("manifest")).expect("json");
    let mut gate = Gate {
        cache: Cache::new(),
        allow: allowed(),
        totals: Totals::default(),
        failures: Vec::new(),
        findings: Map::new(),
        set_aside: Vec::new(),
    };
    for p in manifest["projects"].as_array().expect("projects") {
        let (abs, name) = (p["abs"].as_str().expect("abs"), p["project"].as_str().expect("name"));
        let abs = support::spell(std::path::Path::new(abs));
        let ours = gate.read(name, &abs, true);
        let theirs = dump_of(&dumps, name);
        gate.compare(name, &ours, &theirs);
        gate.modules(name, &theirs, true);
        gate.count(&ours);
        let seen: Vec<Value> = ["clips", "sequences", "timelines"]
            .iter()
            .filter(|s| !finding_files(&ours, s).is_empty())
            .map(|s| json!([s, finding_files(&ours, s)]))
            .collect();
        if !seen.is_empty() {
            gate.findings.insert(name.to_owned(), json!(seen));
        }
        gate.totals.projects += 1;
    }
    // The negative corpus the TypeScript can read: both sides must give the same files, and the expected ones.
    for case in cases().iter().filter(|c| c.ts) {
        let base = staged.join("negative").join(&case.name);
        let abs = support::spell(&if case.module { base.join("ws/proj") } else { base });
        let name = format!("negative/{}", case.name);
        let theirs = dump_of(&dumps, &name);
        if case.module {
            gate.modules(&name, &theirs, case.mem);
            if case.routing {
                gate.count_negative_routing(case);
            }
            gate.totals.module_negative += 1;
            gate.totals.resolve_negative += usize::from(case.resolve);
            continue;
        }
        let ours = gate.read(&name, &abs, case.mem);
        check_expected(&mut gate, case, &ours);
        gate.compare(&name, &ours, &theirs);
        gate.totals.negative += 1;
        gate.totals.negative_timeline_findings += finding_files(&ours, "timelines").len();
    }
    // The routing negative cases the TypeScript cannot read are counted too: only they carry the DEPARTS.
    for case in cases().iter().filter(|c| c.routing && !c.ts) {
        gate.count_negative_routing(case);
    }
    let t = &gate.totals;
    eprintln!(
        "stage gate: {} projects ({} remembered through a bundle), {} clips, {} sequences, {} timelines, {} markers, \
         {} files with findings, {} families the TypeScript threw on, {} negative cases",
        t.projects, t.remembered, t.clips, t.sequences, t.timelines, t.markers, t.finding_files, t.threw, t.negative
    );
    eprintln!(
        "modules: {} projects ({} through a bundle), {} nodes, {} library modules, {} category modules, {} engines, \
         {} library nodes, {} library findings, {} scan plugins, {} module negative cases",
        t.module_projects,
        t.module_remembered,
        t.mods.nodes,
        t.seen.library.len(),
        t.seen.category.len(),
        t.seen.engines.len(),
        t.seen.nodes.len(),
        t.mods.library_findings,
        t.mods.scan_plugins,
        t.module_negative
    );
    eprintln!(
        "links: {} family folders that are a link out, refused and replayed as refused",
        t.linked_out
    );
    let r = &t.res;
    eprintln!(
        "resolved: {} panels, {} scripts, {} maps, {} presets, {} rack clips, {} file params, {} project samples, \
         {} extends folds, {} alternatives, {} engines, {} not-supported, {} unknown-plugin findings",
        r.panels.len(),
        r.scripts.len(),
        r.maps.len(),
        r.presets.len(),
        r.clips.len(),
        r.files.len(),
        r.project_samples.len(),
        r.folds.len(),
        r.alternatives.len(),
        r.engines.len(),
        r.not_supported,
        r.unknown_plugin
    );
    eprintln!(
        "resolution weight: at most {} of {}",
        r.max_weight,
        song::resolve::BUDGET
    );
    let q = &t.route;
    eprintln!(
        "routing: {} chain projects ({} chain files, {} nodes; {}, {}, {} of the staged corpus), {} graph projects, {} subchain nodes, {} picture lanes, \
         {} levels, {} bypassed, {} bus-only, {} port keys, {} port destinations; negative corpus: {} cases ({} ts), \
         {} tap feeds, {} positions, {} hidden chains, {} call steps, {} parallel steps, {} runs",
        q.chain_projects,
        q.chain_files,
        q.chain_nodes,
        q.staged_projects,
        q.staged_files,
        q.staged_nodes,
        q.graph_segments,
        q.subchain_nodes,
        q.picture_lanes,
        q.levels,
        q.bypassed,
        q.bus_only,
        q.port_keys,
        q.port_dests,
        q.negative,
        q.negative_ts,
        q.tap_feeds,
        q.positions,
        q.hidden,
        q.calls,
        q.parallels,
        q.runs
    );
    eprintln!("routing budget: at most {} of {}", q.max_weight, song::routing::BUDGET);
    if q.max_weight.saturating_mul(10) > song::routing::BUDGET {
        gate.failures.push(format!(
            "a routing call spent {} of the {} budget: past a tenth",
            q.max_weight,
            song::routing::BUDGET
        ));
    }
    let hits = gate.cache.vocabulary_hits();
    eprintln!("vocabulary cache: {hits} hits, {} checks skipped", support::skipped());
    let mins = [
        (
            "chain projects with their nodes compared",
            q.staged_projects,
            MIN_CHAIN_PROJECTS,
        ),
        ("chain files of the staged corpus", q.staged_files, MIN_CHAIN_FILES),
        ("chain nodes of the staged corpus", q.staged_nodes, MIN_CHAIN_NODES),
        (
            "graph projects' segments compared",
            q.graph_segments,
            MIN_GRAPH_SEGMENTS,
        ),
        ("subchain nodes", q.subchain_nodes, MIN_SUBCHAIN_NODES),
        ("picture lanes", q.picture_lanes, MIN_PICTURE_LANES),
        ("link levels applied", q.levels, MIN_LEVELS),
        ("bypassed modules", q.bypassed, MIN_BYPASSED),
        ("bus-only chains", q.bus_only, 1),
        ("port keys", q.port_keys, 1),
        ("port destinations", q.port_dests, 1),
        ("tap feeds", q.tap_feeds, 1),
        ("positions", q.positions, 1),
        ("hidden chains", q.hidden, 1),
        ("call steps", q.calls, 1),
        ("parallel steps", q.parallels, 1),
        ("`runs` graphs", q.runs, 1),
        ("routing negative cases", q.negative, 40),
        ("routing negative cases the TypeScript reads", q.negative_ts, 30),
        ("vocabulary cache hits", hits, MIN_VOCABULARY_HITS),
        ("resolution negative cases", t.resolve_negative, MIN_RESOLVE_NEGATIVE),
        ("panels stamped", r.panels.len(), MIN_PANELS),
        ("modules with a script", r.scripts.len(), MIN_SCRIPTS),
        ("MIDI maps stamped", r.maps.len(), MIN_MAPS),
        ("presets found", r.presets.len(), MIN_PRESETS),
        ("modules with a rack clip resolved", r.clips.len(), MIN_RESOLVED_CLIPS),
        ("modules with a file param resolved", r.files.len(), MIN_FILES),
        (
            "`@project/` samples resolved",
            r.project_samples.len(),
            MIN_PROJECT_SAMPLES,
        ),
        ("extends folds", r.folds.len(), MIN_FOLDS),
        ("alternatives applied", r.alternatives.len(), MIN_ALTERNATIVES),
        ("engines run", r.engines.len(), MIN_ENGINES_RUN),
        ("not-supported findings", r.not_supported, 1),
        ("unknown-plugin findings", r.unknown_plugin, 1),
        ("module negative cases", t.module_negative, MIN_MODULE_NEGATIVE),
        (
            "projects read again through a bundle for their modules",
            t.module_remembered,
            t.module_projects - t.module_unbundled,
        ),
        (
            "library family folders that are a link out, refused and replayed as refused (743)",
            t.linked_out,
            1,
        ),
        ("nodes", t.mods.nodes, MIN_NODES),
        ("library modules", t.seen.library.len(), MIN_LIBRARY_MODULES),
        ("category modules", t.seen.category.len(), MIN_CATEGORY_MODULES),
        ("engines", t.seen.engines.len(), MIN_ENGINES),
        ("library nodes", t.seen.nodes.len(), MIN_LIBRARY_NODES),
        ("library findings", t.mods.library_findings, MIN_LIBRARY_FINDINGS),
        ("scan plugins", t.mods.scan_plugins, MIN_SCAN_PLUGINS),
        ("projects", t.projects, MIN_PROJECTS),
        ("clips", t.clips, MIN_CLIPS),
        ("sequences", t.sequences, MIN_SEQUENCES),
        ("markers", t.markers, MIN_MARKERS),
        ("files with findings", t.finding_files, MIN_FINDING_FILES),
        ("negative cases", t.negative, MIN_NEGATIVE),
        ("negative timeline findings", t.negative_timeline_findings, 1),
        ("projects read again through a bundle", t.remembered, MIN_PROJECTS),
    ];
    // The first green run's maximum must stay ten times below the budget, or the budget protects nothing.
    if r.max_weight.saturating_mul(10) > song::resolve::BUDGET {
        gate.failures.push(format!(
            "a call spent {} of the {} budget: past a tenth",
            r.max_weight,
            song::resolve::BUDGET
        ));
    }
    for (what, got, min) in mins {
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
    insta::assert_json_snapshot!(
        "stage_gate_findings_and_allowed",
        json!({ "findings": gate.findings, "allowed": gate.set_aside })
    );
}

/// The files the case says must carry findings are exactly the ones the Rust named.
fn check_expected(gate: &mut Gate, case: &Case, ours: &Value) {
    for sub in ["clips", "sequences", "timelines"] {
        let mut want: Vec<String> = case.expect[sub]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        want.sort();
        if finding_files(ours, sub) != want {
            gate.failures
                .push(format!("negative/{} {sub}: expected {want:?}", case.name));
        }
    }
}
