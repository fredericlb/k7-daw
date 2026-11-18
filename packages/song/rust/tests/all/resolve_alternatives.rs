// resolve_alternatives.rs — a module's attribute sets: alternatives.test.ts, and decision 58 (applied once, after the fold).
// A reader would get wrong that a set naming another module SWAPS the node into it, except for a lane and the mixer's fader.

use crate::support;

use indexmap::IndexMap;
use song::modules::{AlternativePick, LoadedModule, ModuleKind, ParamValue, decode_module};
use song::resolve::{alternatives_of, current_alternative, pick_alternative};
use support::{by_ref, node_of, resolve, resolve_one};

fn param(m: &LoadedModule, k: &str) -> Option<f64> {
    match m.authored.params.as_ref()?.get(k)? {
        ParamValue::Number(n) => Some(*n),
        ParamValue::Text(_) => None,
    }
}

const SYNTH: &str = "plugin({ type: \"internal\", name: \"Synth\" })\nparams({ cutoff: 1000, res: 0.2 })\nalternatives([{ name: \"dark\", params: { cutoff: 200 } }, { params: { res: 0.9 } }])\n";

// alternatives.test.ts, "applying one": a set states only what it changes, params merge per entry.
#[test]
fn a_set_merges_params_per_entry() {
    let m = resolve_one(&format!("{SYNTH}alternative(\"dark\")\n"), &[]);
    assert_eq!((param(&m, "cutoff"), param(&m, "res")), (Some(200.0), Some(0.2)));
    let third = resolve_one(&format!("{SYNTH}alternative(3)\n"), &[]);
    assert_eq!(
        (param(&third, "cutoff"), param(&third, "res")),
        (Some(1000.0), Some(0.9))
    );
}

// alternatives.test.ts, "replaces a LIST outright", "may set ANY module field", "does NOT rename the module", "leaves the list itself".
#[test]
fn a_set_replaces_a_list_sets_any_field_and_leaves_the_module_its_name_and_list() {
    let text = "tracks([\"a\", \"b\"])\nalternatives([{ name: \"b\", tracks: [], gain: 4, color: \"#fff\" }])\nalternative(\"b\")\n";
    let mut node = node_of("n", text);
    node.name = Some("n".into());
    let modules: IndexMap<String, LoadedModule> = [("n".to_owned(), node)].into_iter().collect();
    let out = resolve(&modules, &IndexMap::new())
        .modules
        .shift_remove("n")
        .expect("resolved");
    assert_eq!(out.authored.tracks.as_ref().map(Vec::len), Some(0));
    assert_eq!(out.authored.gain, Some(4.0));
    assert_eq!(out.name.as_deref(), Some("n"));
    assert_eq!(out.authored.alternatives.as_ref().map(Vec::len), Some(1));
}

// alternatives.test.ts, "is a no-op when the first is on, or when the name answers to nothing".
#[test]
fn the_first_is_the_modules_own_and_a_name_that_answers_nothing_falls_back_to_it() {
    for pick in ["alternative(1)\n", "alternative(\"nope\")\n", "alternative(9)\n", ""] {
        let m = resolve_one(&format!("{SYNTH}{pick}"), &[]);
        assert_eq!(param(&m, "cutoff"), Some(1000.0), "{pick}");
    }
    let module = decode_module("m.k7x", &format!("{SYNTH}alternative(\"dark\")\n")).expect("module");
    assert_eq!(
        alternatives_of(&module)
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["1", "dark", "3"]
    );
    assert_eq!(current_alternative(&module), 1);
    let names: Vec<String> = ["1", "2", "x"].into_iter().map(String::from).collect();
    // A NAME wins over a number: the alternative literally called "2" is the third.
    assert_eq!(pick_alternative(&names, Some(&AlternativePick::Index(2))), 1);
    assert_eq!(pick_alternative(&names, Some(&AlternativePick::Name("x".into()))), 2);
    assert_eq!(pick_alternative(&names, Some(&AlternativePick::Name(" 3 ".into()))), 2);
    assert_eq!(pick_alternative(&names, Some(&AlternativePick::Name("1.5".into()))), 0);
    assert_eq!(pick_alternative(&names, None), 0);
}

// alternatives.test.ts, "against `extends`": the ancestry is folded FIRST, then the chosen set; a parent's list and choice count.
#[test]
fn the_ancestry_is_folded_before_the_chosen_set() {
    let refs = [("@x/audio/synth", SYNTH)];
    let m = resolve_one(
        "extends(\"@x/audio/synth\")\nparams({ cutoff: 500 })\nalternative(\"dark\")\n",
        &refs,
    );
    assert_eq!(param(&m, "cutoff"), Some(200.0), "the set wins over the child's own");
    let own = resolve_one("extends(\"@x/audio/synth\")\nalternative(3)\n", &refs);
    assert_eq!(param(&own, "res"), Some(0.9), "a list its PARENT declared");
    let parents_choice = [("@x/audio/synth", &*format!("{SYNTH}alternative(\"dark\")\n"))];
    let kept = resolve_one("extends(\"@x/audio/synth\")\n", &parents_choice);
    assert_eq!(
        param(&kept, "cutoff"),
        Some(200.0),
        "the parent's own choice, when the child makes none"
    );
}

const SWAP: &str = "extends(\"@x/audio/base\")\nparams({ own: 1 })\nalternatives([{ name: \"other\", extends: \"@x/audio/other\", params: { set: 5 } }])\n";

fn bases() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "@x/audio/base",
            "plugin({ type: \"internal\", name: \"Base\" })\nparams({ b: 1 })\n",
        ),
        (
            "@x/audio/other",
            "plugin({ type: \"internal\", name: \"Other\" })\nparams({ o: 2 })\n",
        ),
    ]
}

// alternatives.test.ts, "a set that names ANOTHER module": the node becomes that module, none of its own params, the list kept.
#[test]
fn a_set_naming_another_module_swaps_the_node_into_it() {
    let m = resolve_one(&format!("{SWAP}alternative(\"other\")\n"), &bases());
    assert_eq!(
        m.authored.plugin.as_ref().and_then(|p| p.name.as_deref()),
        Some("Other")
    );
    assert_eq!(
        (param(&m, "o"), param(&m, "set"), param(&m, "own"), param(&m, "b")),
        (Some(2.0), Some(5.0), None, None)
    );
    assert_eq!(m.authored.alternatives.as_ref().map(Vec::len), Some(1));
    assert!(m.authored.alternative.is_some(), "the choice stays");
    // "is the node's own module again on set 1".
    let back = resolve_one(&format!("{SWAP}alternative(1)\n"), &bases());
    assert_eq!(
        back.authored.plugin.as_ref().and_then(|p| p.name.as_deref()),
        Some("Base")
    );
    assert_eq!(param(&back, "own"), Some(1.0));
}

// alternatives.test.ts, "does not swap a LANE" and "a lane's FADER": the arrangement and the mixer are built on them.
#[test]
fn a_lane_and_the_mixers_fader_keep_their_module() {
    let mut refs = by_ref(&bases());
    refs.get_mut("@x/audio/base").expect("base").kind = Some(ModuleKind::Track);
    let node: IndexMap<String, LoadedModule> =
        [("n".to_owned(), node_of("n", &format!("{SWAP}alternative(\"other\")\n")))]
            .into_iter()
            .collect();
    let lane = resolve(&node, &refs).modules.shift_remove("n").expect("resolved");
    assert_eq!(
        lane.authored.plugin.as_ref().and_then(|p| p.name.as_deref()),
        Some("Base")
    );
    assert_eq!(param(&lane, "set"), Some(5.0), "its settings still switch, as a patch");
    let fader = resolve_one(
        "plugin({ type: \"internal\", name: \"Mixer\" })\nalternatives([{ name: \"x\", extends: \"@x/audio/other\", params: { v: 1 } }])\nalternative(\"x\")\n",
        &bases(),
    );
    assert_eq!(
        fader.authored.plugin.as_ref().and_then(|p| p.name.as_deref()),
        Some("Mixer")
    );
    assert_eq!(param(&fader, "v"), Some(1.0));
    assert_eq!(param(&fader, "o"), None);
}

// Decision 87: a pick is read as JavaScript's `Number` reads it, hex included.
#[test]
fn a_pick_is_read_as_javascript_reads_a_number() {
    let names: Vec<String> = ["1", "a", "b"].into_iter().map(String::from).collect();
    for (pick, at) in [
        ("0x2", 1),
        ("0X3", 2),
        ("0b10", 1),
        ("0o3", 2),
        (" 2 ", 1),
        ("1e0", 0),
        ("0x9", 0),
    ] {
        assert_eq!(
            pick_alternative(&names, Some(&AlternativePick::Name(pick.into()))),
            at,
            "{pick}"
        );
    }
}
