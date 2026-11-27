// doc_clip_ids.rs — a clip placed more than once on a lane gets one id per placement, stable from one compile to the next.
// A reader would get wrong that the ref is the clip's id: it is `<ref>@<start>`, and a placement repeated at one beat adds `#2`, `#3`.

use crate::doc_support;

use doc_support::{AMP, compiled, lane};
use song::clips::{material_ref_of, without_repeat};
use song::doc::{Compiled, ProjectDoc};
use song::files::Severity;

const CLIP: &str = "source(\"k.wav\")\n";

fn files_with(lanes: &[(&str, String)]) -> Compiled {
    let mut files: Vec<(&str, String)> = vec![
        ("clips/k.k7x", CLIP.to_owned()),
        ("clips/h.k7x", CLIP.to_owned()),
        ("k.wav", "w".to_owned()),
        ("nodes/amp.k7x", AMP.to_owned()),
    ];
    files.extend(lanes.iter().cloned());
    let names: Vec<String> = lanes
        .iter()
        .filter_map(|(p, _)| p.strip_prefix("nodes/")?.strip_suffix(".k7x").map(str::to_owned))
        .collect();
    let mut modules: Vec<String> = names.iter().map(|n| format!("{n}: [\"amp\"]")).collect();
    modules.push("amp: [\"MIX\"]".to_owned());
    let (g, text) = doc_support::graph(&modules.join(", "));
    files.push((g, text));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    compiled(&borrowed)
}

fn doc(c: &Compiled) -> &ProjectDoc {
    c.engine
        .as_ref()
        .unwrap_or_else(|f| panic!("the compile failed: {:?}", f.diagnostics))
}

fn clip_ids(d: &ProjectDoc, track: &str) -> Vec<String> {
    let t = d.tracks.iter().find(|t| t.id == track).expect("the track");
    t.clips.iter().map(|c| c.id.clone()).collect()
}

fn twin_warnings(c: &Compiled) -> Vec<String> {
    c.diagnostics
        .iter()
        .filter(|f| f.severity == Severity::Warning && f.message.contains("is placed twice"))
        .map(|f| f.message.clone())
        .collect()
}

// The real project's case: one ref three times, twice at bar 6, the second with its from/to merely written in swapped order.
const REPEATS: &str = "events([{ ref: \"k\", start: \"6:1\", from: 4.5, to: 45.5 }, { ref: \"h\", start: \"25:1\" }, { ref: \"k\", start: \"6:1\", to: 45.5, from: 4.5 }, { ref: \"k\", start: \"35:1\", from: 6.5, to: 45.5 }])\n";

#[test]
fn a_ref_placed_twice_at_one_beat_gets_a_second_id() {
    let c = files_with(&[("nodes/gtr.k7x", lane(REPEATS))]);
    assert_eq!(clip_ids(doc(&c), "gtr"), ["k@20", "h@96", "k@20#2", "k@136"]);
    assert_eq!(material_ref_of("k@20#2"), "k");
    assert_eq!(song::clips::audio_def::material_ref_of("v@8>k@20#2"), "k");
    assert_eq!(without_repeat("k@20#2"), "k@20");
}

#[test]
fn the_ids_are_the_same_from_one_compile_to_the_next() {
    let files = [("nodes/gtr.k7x", lane(REPEATS))];
    assert_eq!(
        clip_ids(doc(&files_with(&files)), "gtr"),
        clip_ids(doc(&files_with(&files)), "gtr")
    );
}

#[test]
fn an_identical_placement_at_one_beat_is_a_warning_naming_where() {
    let c = files_with(&[("nodes/gtr.k7x", lane(REPEATS))]);
    assert_eq!(
        twin_warnings(&c),
        ["the same clip 'k' is placed twice at 6:1 on lane 'gtr'"]
    );
    assert!(c.engine.is_ok(), "a warning does not stop the compile");
}

#[test]
fn a_second_placement_with_another_window_is_no_warning() {
    let text = lane("events([{ ref: \"k\", start: \"6:1\", from: 1 }, { ref: \"k\", start: \"6:1\", from: 2 }])\n");
    let c = files_with(&[("nodes/gtr.k7x", text)]);
    assert_eq!(clip_ids(doc(&c), "gtr"), ["k@20", "k@20#2"]);
    assert!(twin_warnings(&c).is_empty());
}

#[test]
fn the_chosen_alternative_numbers_its_own_repeats() {
    let text = lane(
        "events([{ ref: \"k\", start: \"1:1\" }])\nalternatives([{ name: \"6\", events: [{ ref: \"k\", start: \"2:1\" }, { ref: \"k\", start: \"2:1\" }] }])\nalternative(\"6\")\n",
    );
    let c = files_with(&[("nodes/gtr.k7x", text)]);
    assert_eq!(clip_ids(doc(&c), "gtr"), ["k@4", "k@4#2"]);
}

#[test]
fn one_ref_on_two_lanes_keeps_its_plain_id_on_each() {
    let one = lane("events([{ ref: \"k\", start: \"1:1\" }])\n");
    let c = files_with(&[("nodes/gtr.k7x", one.clone()), ("nodes/drums.k7x", one)]);
    assert_eq!(clip_ids(doc(&c), "gtr"), ["k@0"]);
    assert_eq!(clip_ids(doc(&c), "drums"), ["k@0"]);
}

#[test]
fn a_repeated_comp_numbers_its_pieces_and_their_event_alike() {
    let comped = "{ ref: \"k\", start: \"1:1\", takes: [{ ref: \"h\", start: \"1:1\" }], comp: [{ take: \"h\", from: \"2:1\", to: \"3:1\" }] }";
    let text = lane(&format!("events([{comped}, {comped}])\n"));
    let c = files_with(&[("nodes/gtr.k7x", text)]);
    let d = doc(&c);
    let t = d.tracks.iter().find(|t| t.id == "gtr").expect("the track");
    let ids: Vec<&str> = t.clips.iter().map(|c| c.id.as_str()).collect();
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "every id once: {ids:?}");
    let ofs: Vec<&str> = t
        .clips
        .iter()
        .filter_map(|c| c.comp.as_ref())
        .map(|m| m.of.as_str())
        .collect();
    assert!(ofs.contains(&"k@0") && ofs.contains(&"k@0#2"), "{ofs:?}");
    let half = ofs.len() / 2;
    assert!(
        ofs[..half].iter().all(|o| *o == "k@0") && ofs[half..].iter().all(|o| *o == "k@0#2"),
        "{ofs:?}"
    );
    assert_eq!(twin_warnings(&c).len(), 1);
}

// placementId.test.ts, "a placement id": the start read back, a repeat's `#n` aside.
#[test]
fn a_placement_id_names_its_start() {
    use song::clips::placement_start_of;
    assert_eq!(placement_start_of("riff@16.5"), Some(16.5));
    assert_eq!(placement_start_of("verse@8>kick@0#3"), Some(0.0));
    assert_eq!(placement_start_of("riff"), None);
    assert_eq!(placement_start_of("@4"), None);
    assert_eq!(placement_start_of("riff@"), None);
    assert_eq!(without_repeat("#2"), "#2");
}
