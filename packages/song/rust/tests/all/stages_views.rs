// stages_views.rs — the stage gate of the views: the Rust against the TypeScript's dumps, the sweeps and the fields lattice.
//
//   K7_SONG_STAGED=<dir> K7_SONG_DUMPS=<dumps/views> cargo test --test all -- --ignored stages_views::
//
// What a reader gets wrong: PARTS names what has landed, so the gate is honest at each stop; a diagnostic set is compared by
// its `file|severity` projection (decision 647), and an empty allow list is the point.
// Ignored by default: it needs the corpus staged by stageCorpus.ts and dumped by scripts/stageDumpViews.ts.

use crate::support;
use crate::views_gate;

use serde_json::Value;
use song::lanes::rig::Rig;
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots};
use std::path::{Path, PathBuf};
use views_gate::sweeps::{clips_gate, lattice_gate, wav_gate};
use views_gate::{Ask, Counts, cases, compare_views, read_json, views};

/// What has landed of the views: a part not listed here is not compared.
const PARTS: [&str; 3] = ["a1", "a2", "b"];

/// The floors: each below the first green run, each the effect of the work and never zero.
const MINIMUMS: [(&str, usize); 3] = [
    ("projects", 170),
    ("negative cases", 90),
    ("negative cases with the TypeScript", 70),
];

fn roots_of(theirs: &Value) -> Roots {
    serde_json::from_value(theirs["roots"].clone()).expect("roots")
}

/// One project to read: its name, the TypeScript's dump of it, the roots and rig to read it with, and whether to replay the bundle.
struct Run<'a> {
    name: &'a str,
    theirs: &'a Value,
    roots: &'a Roots,
    rig: &'a Rig,
    vsts_none: bool,
    mem: bool,
}

/// The views of one project through a Recorder, again through the bundle it made, and against the dump.
fn one(run: &Run<'_>, cache: &mut Cache, f: &mut Vec<String>, c: &mut Counts) {
    let Run {
        name,
        theirs,
        roots,
        rig,
        vsts_none,
        mem,
    } = *run;
    let ask = Ask { roots, rig, vsts_none };
    let (mounts, files) = roots.mounts().expect("mounts");
    let disk = DiskSource::with_files(&mounts, &files).expect("source");
    let rec = Recorder::new(&disk);
    let (ours, lengths) = match views(&rec, cache, &ask, None) {
        Ok(v) => v,
        Err(e) => {
            f.push(format!("{name}: {e}"));
            return;
        }
    };
    if mem {
        let bundle = rec.into_bundle_with_files(&mounts, &files);
        // The replay draws from the table the first pass measured: a bundle holds a header by its length alone (decision 611).
        match MemSource::from_bundle(bundle).map(|m| views(&m, &mut Cache::new(), &ask, Some(&lengths))) {
            Ok(Ok((again, _))) if again == ours => c.bundles += 1,
            Ok(Ok(_)) => f.push(format!(
                "{name}: the bundle read differently from the disk (a risk for 2l)"
            )),
            Ok(Err(e)) => f.push(format!("{name}: the bundle was not enough: {e} (a risk for 2l)")),
            Err(e) => f.push(format!("{name}: {e}")),
        }
    }
    compare_views(f, name, theirs, &ours, c);
}

fn project_gate(staged: &Path, dumps: &Path, failures: &mut Vec<String>, c: &mut Counts) {
    let manifest = read_json(&staged.join("manifest.json"));
    // One Cache shared across the projects, as a long-lived host does.
    let mut cache = Cache::new();
    for p in manifest["projects"].as_array().expect("projects") {
        let name = p["project"].as_str().expect("name");
        let theirs = read_json(&dumps.join(format!("{}.json", name.replace('/', "__"))));
        let roots = roots_of(&theirs);
        let run = Run {
            name,
            theirs: &theirs,
            roots: &roots,
            rig: &Rig::default(),
            vsts_none: false,
            mem: true,
        };
        one(&run, &mut cache, failures, c);
        c.projects += 1;
    }
}

/// The negative cases the TypeScript can answer: laid out as module cases and read with the case's own roots and rig.
fn negative_gate(staged: &Path, dumps: &Path, failures: &mut Vec<String>, c: &mut Counts) {
    for case in cases() {
        c.negative += 1;
        if !case.ts {
            continue;
        }
        c.negative_ts += 1;
        let base = staged.join("negative").join(format!("views-{}", case.name));
        let name = format!("negative-views/{}", case.name);
        let theirs = read_json(&dumps.join(format!("{}.json", name.replace('/', "__"))));
        // The TypeScript reads the real `@meta` tree, so the Rust is given the same one.
        let roots = Roots {
            meta: roots_of(&theirs).meta,
            ..support::case_roots(&base, None)
        };
        let run = Run {
            name: &name,
            theirs: &theirs,
            roots: &roots,
            rig: &case.rig(),
            vsts_none: case.vsts_none,
            mem: case.mem,
        };
        one(&run, &mut Cache::new(), failures, c);
    }
}

#[test]
#[ignore = "needs the staged corpus: K7_SONG_STAGED and K7_SONG_DUMPS"]
fn the_views_agree_with_the_typescript() {
    let staged = PathBuf::from(std::env::var("K7_SONG_STAGED").expect("K7_SONG_STAGED"));
    let dumps = PathBuf::from(std::env::var("K7_SONG_DUMPS").expect("K7_SONG_DUMPS"));
    assert!(["a1", "a2", "b"].iter().all(|p| PARTS.contains(p)));
    let (mut failures, mut c) = (Vec::new(), Counts::default());
    project_gate(&staged, &dumps, &mut failures, &mut c);
    eprintln!("the golden corpus alone:\n{}", c.clone().report());
    for (what, got, min) in c.web.golden_floors() {
        if got < min {
            failures.push(format!("the golden corpus alone showed {got} {what}, fewer than {min}"));
        }
    }
    negative_gate(&staged, &dumps, &mut failures, &mut c);
    lattice_gate(&dumps, &mut failures, &mut c);
    wav_gate(&dumps, &mut failures, &mut c);
    clips_gate(&dumps, &mut failures, &mut c);
    eprintln!("{}", c.report());
    for (what, got, min) in c
        .floors()
        .into_iter()
        .chain(MINIMUMS.map(|(w, m)| (w, got_of(&c, w), m)))
    {
        if got < min {
            failures.push(format!(
                "the gate saw {got} {what}, fewer than {min}: it proves nothing"
            ));
        }
    }
    // A limit that stopped a real project is a limit too small: the most any view spent stays ten times under the budget.
    for (i, what) in ["hints", "io ports", "catalogue", "base modules"].iter().enumerate() {
        if c.max_spent[i] * 10 >= song::views::BUDGET {
            failures.push(format!(
                "{what} spent {}: not ten times under the budget",
                c.max_spent[i]
            ));
        }
    }
    if c.capped > 0 {
        failures.push(format!(
            "{} answers were capped on projects that are not hostile",
            c.capped
        ));
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn got_of(c: &Counts, what: &str) -> usize {
    match what {
        "projects" => c.projects,
        "negative cases" => c.negative,
        _ => c.negative_ts,
    }
}

#[test]
fn the_null_check_flags_a_null_and_lets_a_mapping_entry_through() {
    let ok = serde_json::json!({"surfaces": {"surfaces": [{"surface": {"mapping": {"k": null}}}]}});
    let bad = serde_json::json!({"hints": {"curves": [null]}, "surfaces": {"surfaces": [{"surface": {"mapping": {"k": 1}, "x": null}}]}});
    let (mut a, mut b) = (Vec::new(), Vec::new());
    views_gate::no_null(&ok, "", &mut a);
    views_gate::no_null(&bad, "", &mut b);
    assert!(a.is_empty(), "{a:?}");
    assert_eq!(b, ["/hints/curves/0", "/surfaces/surfaces/0/surface/x"]);
}
