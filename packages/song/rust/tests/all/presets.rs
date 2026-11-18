// presets.rs — the state file a module loads: basePreset.test.ts, moduleCategories.test.ts and decisions 33, 65, 69.
// A reader would get wrong that a preset is only probed: the PROBED spelling answers, and a folder answers as absent.

use crate::support;

use song::Cache;
use song::modules::preset::is_surge_patch;
use support::{Tmp, disk, folds_case, mem_world, roots, skip};

const AMP: &str = "lib/k7-core/modules/audio/amp";
const MOD: &str = "plugin({ type: \"internal\", name: \"Amp\" })\n";

fn p(rel: &str) -> String {
    format!("/p/{rel}")
}

fn preset_of(w: &support::World, key: &str) -> Option<String> {
    w.vocab.modules[key]
        .preset
        .clone()
        .or_else(|| w.resolved.modules.get(key).and_then(|m| m.preset.clone()))
}

// basePreset.test.ts, "prefers the CHOSEN base over both defaults", "falls back to the node's own, then the module's".
#[test]
fn the_chosen_base_beats_the_nodes_own_state_and_that_beats_the_modules() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/amp.vstpreset"), "x"),
        (&format!("{AMP}/lead.k7x"), "base(\"Warm\")\n"),
        (&format!("{AMP}/lead.vstpreset"), "x"),
        (&format!("{AMP}/base/Warm.vstpreset"), "x"),
        (&format!("{AMP}/pad.k7x"), "params({ a: 1 })\n"),
        (&format!("{AMP}/pad.vstpreset"), "x"),
        (&format!("{AMP}/plain.k7x"), "params({ a: 2 })\n"),
    ]);
    assert_eq!(
        preset_of(&w, "@k7-core/audio/amp"),
        Some(p(&format!("{AMP}/amp.vstpreset")))
    );
    assert_eq!(
        preset_of(&w, "@k7-core/audio/amp:lead"),
        Some(p(&format!("{AMP}/base/Warm.vstpreset")))
    );
    assert_eq!(
        preset_of(&w, "@k7-core/audio/amp:pad"),
        Some(p(&format!("{AMP}/pad.vstpreset")))
    );
    assert_eq!(
        preset_of(&w, "@k7-core/audio/amp:plain"),
        Some(p(&format!("{AMP}/amp.vstpreset")))
    );
}

// basePreset.test.ts, "resolves a nested base to its .fxp" and "falls back rather than loading nothing when the base has gone missing".
#[test]
fn a_nested_base_resolves_to_its_fxp_and_a_missing_one_falls_back() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/amp.vstpreset"), "x"),
        (&format!("{AMP}/base/Pads/Bell.fxp"), "x"),
        (&format!("{AMP}/bell.k7x"), "base(\"Pads/Bell\")\n"),
        (&format!("{AMP}/gone.k7x"), "base(\"Nope\")\n"),
    ]);
    let bell = preset_of(&w, "@k7-core/audio/amp:bell");
    assert_eq!(bell, Some(p(&format!("{AMP}/base/Pads/Bell.fxp"))));
    assert!(is_surge_patch(bell.as_deref()) && !is_surge_patch(None));
    assert_eq!(
        preset_of(&w, "@k7-core/audio/amp:gone"),
        Some(p(&format!("{AMP}/amp.vstpreset")))
    );
}

// basePreset.test.ts, "refuses a base name that climbs out of base/", and the hostile bases of decision 69.
#[test]
fn a_base_that_escapes_or_is_absurd_falls_back_to_the_defaults() {
    let long = "x".repeat(10_000);
    for base in [
        "../../../x",
        "../amp",
        "/etc/passwd",
        "a/../../b",
        long.as_str(),
        "nul\\u0000byte",
    ] {
        let w = mem_world(&[
            (&format!("{AMP}/module.k7x"), MOD),
            (&format!("{AMP}/amp.vstpreset"), "x"),
            (&format!("{AMP}/outside.vstpreset"), "x"),
            (&format!("{AMP}/k.k7x"), &format!("base(\"{base}\")\n")),
        ]);
        assert_eq!(
            preset_of(&w, "@k7-core/audio/amp:k"),
            Some(p(&format!("{AMP}/amp.vstpreset"))),
            "{}",
            &base[..base.len().min(20)]
        );
    }
}

// basePreset.test.ts, "is undefined when the module has no state at all", and decision 69: a folder answers as absent.
#[test]
fn no_state_is_no_preset_and_a_folder_named_like_one_is_absent() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/amp.vstpreset/inner"), "x"),
    ]);
    assert_eq!(preset_of(&w, "@k7-core/audio/amp"), None);
}

// moduleCategories.test.ts, "still finds the preset beside a categorised module".
#[test]
fn a_categorised_module_finds_its_preset() {
    let w = mem_world(&[
        ("lib/k7-core/modules/audio/delay/adt/module.k7x", MOD),
        ("lib/k7-core/modules/audio/delay/adt/adt.vstpreset", "x"),
    ]);
    assert_eq!(
        preset_of(&w, "@k7-core/audio/delay/adt"),
        Some(p("lib/k7-core/modules/audio/delay/adt/adt.vstpreset"))
    );
}

// Decision 33 and 69 over MemSource: names are compared case-sensitively, so only the probed spelling answers.
#[test]
fn only_the_probed_spelling_answers_in_memory() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/AMP.vstpreset"), "x"),
    ]);
    assert_eq!(preset_of(&w, "@k7-core/audio/amp"), None);
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/amp.vstpreset"), "x"),
    ]);
    assert!(preset_of(&w, "@k7-core/audio/amp").is_some_and(|s| s.ends_with("/amp.vstpreset")));
}

// Decision 33 on disk: where the file system folds case the probe finds the other spelling and answers with the PROBED one.
#[test]
fn on_disk_the_file_systems_case_rule_decides_and_the_probed_spelling_answers() {
    let t = Tmp::new("preset-case");
    t.write(&format!("{AMP}/module.k7x"), MOD);
    t.write(&format!("{AMP}/AMP.vstpreset"), "x");
    t.mkdir("ws/proj");
    t.age();
    let r = roots(&t);
    let w = support::world(&disk(&r), &mut Cache::new(), &r).expect("a world");
    let got = w.vocab.modules["@k7-core/audio/amp"].preset.clone();
    if folds_case(&t) {
        assert_eq!(
            got,
            Some(t.at(&format!("{AMP}/amp.vstpreset"))),
            "the probed spelling, not the file's"
        );
    } else {
        assert_eq!(got, None);
        skip(
            "on_disk_the_file_systems_case_rule_decides_and_the_probed_spelling_answers",
            "this file system keeps case: the folding branch is not reachable",
        );
    }
}

// Decision 69 on Windows: a preset path past 260 characters still resolves.
#[test]
fn a_preset_path_past_260_characters_resolves() {
    let t = Tmp::new("preset-long");
    let leaf = "m".repeat(120);
    let dir = format!("lib/k7-core/modules/audio/{leaf}");
    t.write(&format!("{dir}/module.k7x"), MOD);
    t.write(&format!("{dir}/{leaf}.vstpreset"), "x");
    t.mkdir("ws/proj");
    t.age();
    let r = roots(&t);
    let w = support::world(&disk(&r), &mut Cache::new(), &r).expect("a world");
    let got = w.vocab.modules[&format!("@k7-core/audio/{leaf}")]
        .preset
        .clone()
        .expect("a preset");
    assert!(got.len() > 260, "{}", got.len());
}
