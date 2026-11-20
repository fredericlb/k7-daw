// graph_negative.rs — the negative corpus of 2k: each case a project laid out as a module case, with the sets and pointers it must give.
// A reader would get wrong that this needs no TypeScript: stages_graph.rs adds the TypeScript's answers to the cases that can have one.

use crate::graph_support;
use crate::support;

use graph_support::{Counts, check, graph_cases, lay, lay_renders, run};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use support::Tmp;

#[test]
fn every_case_gives_what_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("graph-negative");
    let mut seen = Counts::default();
    let mut failures = Vec::new();
    for case in graph_cases() {
        let base = t.0.join(&case.name);
        lay(&base, &case);
        let roots = case.roots(&base);
        if case.renders.as_deref() == Some("ready") {
            lay_renders(&roots, &case.rig);
        }
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = run(&rec, &mut Cache::new(), &roots, &case.rig, case.via())
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        failures.extend(check(&case, &ran));
        seen.of(&ran);
        if case.mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            let again = run(&mem, &mut Cache::new(), &roots, &case.rig, case.via())
                .unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
            if !again.same_as(&ran) {
                failures.push(format!("{}: the bundle read differently from the disk", case.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // Every code a file can make appears in the corpus, and the kinds of refusal and finding it holds.
    for code in song::graph::WarningCode::ALL {
        let name = code.name();
        // Two codes need a state no file can make; they are raised on a changed graph in graph_validate.rs.
        if matches!(name, "outNegativeGain" | "chainDeadEnd") {
            continue;
        }
        assert!(seen.get(&format!("issue {name}")) > 0, "no case raises {name}");
    }
    for (what, key, min) in [
        ("refusals", "refused", 4),
        ("rack kits", "step key rackKit", 1),
        ("stubs", "stub true", 1),
        ("automations with a trigger", "automations with a trigger", 1),
        ("curves", "top curves", 3),
        ("assemblies", "top assemblies", 1),
        ("frozen marks", "top frozen", 1),
        ("auto marks", "top autoFrozen", 1),
        ("auto waiting", "top autoFreezeWaiting", 1),
        ("positions", "top positions", 1),
        ("stacks", "top stacks", 1),
        ("tap feeds", "top tapFeeds", 1),
        ("lane alternatives", "top trackAlternatives", 1),
        ("model downloads", "step key modelDownload", 1),
        ("amp models", "step key ampModel", 3),
        ("bases", "step key bases", 1),
        ("shaders", "step key shaders", 1),
        ("cameras", "step key camera", 1),
        ("devices", "step key device", 1),
        ("taps", "step key tap", 1),
        ("channels", "step key channel", 1),
        ("cc maps or defaults", "step key assetChoices", 3),
        ("manual plans handed_of made", "real handed manual plans", 8),
        ("automatic freezes that play their render", "real auto ready", 1),
        ("automatic freezes the engine hosts", "real auto hosted", 1),
        ("automatic freezes waiting", "real auto waiting", 1),
        ("autoFrozen marks over a real render", "real top autoFrozen", 1),
        ("projects with no compile", "real uncompiled", 5),
    ] {
        assert!(
            seen.get(key) >= min,
            "the corpus has {} {what}, fewer than {min}",
            seen.get(key)
        );
    }
    let cases = graph_cases();
    assert!(cases.len() >= 100 && cases.iter().filter(|c| c.ts).count() >= 88);
}
