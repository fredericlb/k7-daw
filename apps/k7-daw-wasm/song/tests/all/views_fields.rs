// views_fields.rs — the editor's field vocabulary, after fields.test.ts and chainPaths.test.ts; keys compared as sets.
// A reader would get wrong that two answers are the user's ruling, not the TypeScript's yet: the curve kinds (decision 608).

use song::views::fields::{FileKind, fields_at, file_kind_for, values_at};
use std::time::{Duration, Instant};

fn set(v: Vec<&'static str>) -> Vec<&'static str> {
    let mut v = v;
    v.sort_unstable();
    v
}

fn sorted(v: &[&'static str]) -> Vec<&'static str> {
    set(v.to_vec())
}

fn bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

const MODULE: [&str; 45] = [
    "extends",
    "plugin",
    "io",
    "inputs",
    "outputs",
    "type",
    "shader",
    "shape",
    "font",
    "align",
    "background",
    "shadow",
    "params",
    "rack",
    "tap_outs",
    "kit",
    "cold",
    "model_url",
    "channel",
    "camera",
    "device",
    "frame",
    "source",
    "monitor",
    "gain",
    "mix",
    "input_volume",
    "output_volume",
    "base",
    "ui",
    "automations",
    "content",
    "events",
    "color",
    "disabled",
    "tracks",
    "strudel",
    "video_source",
    "lut",
    "frozen",
    "engine",
    "engines",
    "alternatives",
    "alternative",
    "status",
];

#[test]
fn the_module_fields_are_the_schemas() {
    // fields.test.ts: matches the hand-kept table, plus the field the table was missing (607, 608).
    let got = set(fields_at(FileKind::Module, &[]));
    assert_eq!(got, sorted(&MODULE));
}

#[test]
fn a_nested_struct_answers_from_its_own_shape() {
    // fields.test.ts: answers for a nested struct, however deep it sits (607).
    assert_eq!(
        set(fields_at(FileKind::Module, &["ui"])),
        sorted(&["cols", "rows", "widgets", "tabs", "vertical"])
    );
    assert_eq!(
        set(fields_at(FileKind::Module, &["ui", "tabs", "0"])),
        sorted(&["name", "widgets"])
    );
    assert!(fields_at(FileKind::Module, &["ui", "tabs", "0", "widgets", "w1"]).contains(&"bind"));
    assert!(fields_at(FileKind::Module, &["ui", "widgets", "w1"]).contains(&"library"));
    assert_eq!(
        set(fields_at(FileKind::Module, &["rack"])),
        sorted(&["rack_type", "source", "symbols"])
    );
    assert_eq!(
        set(fields_at(FileKind::Module, &["rack", "symbols", "BD"])),
        sorted(&["note", "sample", "clip", "from", "to", "mute"])
    );
}

#[test]
fn nothing_is_offered_inside_an_open_map() {
    // fields.test.ts: offers nothing inside an open map.
    assert!(fields_at(FileKind::Module, &["params"]).is_empty());
    assert!(fields_at(FileKind::Module, &["plugin"]).is_empty());
}

#[test]
fn a_lane_is_a_module_file() {
    // fields.test.ts: has no kind of its own — a lane file is a module file.
    assert_eq!(file_kind_for("nodes/melody.k7x"), Some(FileKind::Module));
    assert_eq!(file_kind_for("tracks/melody.k7x"), Some(FileKind::Module));
    let f = fields_at(FileKind::Module, &[]);
    assert!(["events", "tracks", "strudel", "color"].iter().all(|k| f.contains(k)));
}

#[test]
fn an_alternative_answers_with_the_modules_own_fields() {
    // fields.test.ts: answers inside an ALTERNATIVE with the module's own fields.
    let inside = fields_at(FileKind::Module, &["alternatives", "0"]);
    assert!(
        ["name", "params", "events", "extends"]
            .iter()
            .all(|k| inside.contains(k))
    );
    assert!(!inside.contains(&"alternatives"));
    assert!(fields_at(FileKind::Module, &["alternatives"]).is_empty());
}

#[test]
fn a_placement_and_a_fade_answer_their_own_words() {
    // fields.test.ts: answers inside a placement; and inside a shot's fade map.
    assert_eq!(
        set(fields_at(FileKind::Module, &["events"])),
        sorted(&[
            "ref",
            "start",
            "from",
            "to",
            "fadeIn",
            "fadeOut",
            "loop",
            "gain",
            "disabled",
            "warp",
            "takes",
            "comp",
            "crossfades",
            "length",
            "animate",
            "speed",
            "effect",
            "silent",
            "pitch"
        ])
    );
    assert_eq!(
        set(fields_at(FileKind::Module, &["events", "fadeIn"])),
        sorted(&["over", "with", "chain"])
    );
}

#[test]
fn values_are_a_closed_set_where_the_schema_has_one() {
    // fields.test.ts: offers a closed set where the schema has one.
    assert_eq!(
        values_at(FileKind::Module, &[], "type"),
        ["plugin", "rack", "video_effect", "video_transition"]
    );
    assert_eq!(values_at(FileKind::Module, &[], "monitor"), ["off", "auto", "on"]);
    assert_eq!(
        values_at(FileKind::Module, &[], "io"),
        ["out", "in", "video_out", "file", "camera", "midi"]
    );
    assert_eq!(
        values_at(FileKind::Module, &["ui", "widgets", "w1"], "source"),
        ["input", "output"]
    );
    assert!(values_at(FileKind::Module, &["ui", "widgets", "w1"], "type").contains(&"dropdown"));
    assert!(values_at(FileKind::Module, &[], "extends").is_empty());
    assert!(values_at(FileKind::Module, &[], "docs").is_empty());
}

#[test]
fn the_chains_file_has_five_sections_and_nothing_inside() {
    // fields.test.ts: offers the file's five sections, and nothing inside them.
    assert_eq!(
        fields_at(FileKind::Chains, &[]),
        ["modules", "bypassed", "bus_only", "tap_feeds", "positions"]
    );
    assert!(fields_at(FileKind::Chains, &["modules"]).is_empty());
}

#[test]
fn file_kinds_by_folder() {
    // chainPaths.test.ts: names a chain file, a lane file and the older segments files (609).
    let kind = file_kind_for;
    assert_eq!(kind("chains/gtr.k7x"), Some(FileKind::Chain));
    assert_eq!(kind("chains.k7x"), Some(FileKind::Chains));
    assert_eq!(kind("chains/verb/graph.k7x"), Some(FileKind::Chains));
    assert_eq!(kind("chains/gtr.yaml"), None);
    assert_eq!(kind("project.yaml"), None);
    assert_eq!(kind("Chains/GTR.K7X"), Some(FileKind::Chain));
    assert_eq!(kind("a\\b\\clips\\x.k7x"), Some(FileKind::Clip));
    assert_eq!(kind("PROJECT.K7X"), Some(FileKind::Project));
    assert_eq!(kind("Modules/x.k7x"), None);
    assert_eq!(kind("/sequences/x"), Some(FileKind::Sequence));
    assert_eq!(kind("xcurves/x.k7x"), None);
    assert_eq!(kind(""), None);
    assert_eq!(kind("chains/.k7x"), None);
    assert_eq!(kind("chains//graph.k7x"), Some(FileKind::Chain));
}

#[test]
fn a_chain_offers_its_keys_then_a_nodes_a_links_and_a_modules() {
    // chainPaths.test.ts: offers a chain's keys, then a node's, a link's and a module's.
    let c = FileKind::Chain;
    assert_eq!(
        fields_at(c, &[]),
        ["start", "ports", "bus_only", "hidden", "input", "steps", "subchains"]
    );
    assert_eq!(fields_at(c, &["subchains"]), ["id", "steps"]);
    let steps: Vec<_> = fields_at(c, &["steps"])
        .into_iter()
        .filter(|f| *f != "track" && *f != "sub")
        .collect();
    assert_eq!(set(fields_at(c, &["subchains", "steps"])), set(steps));
    assert_eq!(fields_at(c, &["subchains", "steps", "to"]), ["chain", "level", "port"]);
    assert!(
        ["id", "track", "to", "bypass", "extends", "params", "sub"]
            .iter()
            .all(|k| fields_at(c, &["steps"]).contains(k))
    );
    assert_eq!(fields_at(c, &["steps", "to"]), ["chain", "level", "port"]);
    assert_eq!(
        set(fields_at(c, &["steps", "events"])),
        set(fields_at(FileKind::Module, &["events"]))
    );
    assert_eq!(values_at(c, &[], "start"), ["true", "MIX", "RENDER"]);
    assert_eq!(values_at(c, &["steps"], "io"), values_at(FileKind::Module, &[], "io"));
    assert_eq!(values_at(c, &["subchains", "steps"], "monitor"), ["off", "auto", "on"]);
}

#[test]
fn the_project_clip_sequence_and_curve_kinds_answer() {
    // fields.ts: projectFields, clipFields, sequenceFields, curveFields (607).
    assert!(fields_at(FileKind::Project, &[]).contains(&"trackOrder"));
    assert!(fields_at(FileKind::Project, &["project"]).contains(&"libraries_from"));
    assert!(fields_at(FileKind::Project, &["markers"]).is_empty());
    let clip = fields_at(FileKind::Clip, &[]);
    assert!(
        ["source", "from", "to", "warp", "onsets", "sound", "optimized", "proxy"]
            .iter()
            .all(|k| clip.contains(k))
    );
    assert_eq!(
        set(fields_at(FileKind::Clip, &["warp"])),
        sorted(&["algorithm", "markers", "disable"])
    );
    assert_eq!(
        set(fields_at(FileKind::Sequence, &["notes"])),
        sorted(&["t", "d", "k", "v", "repeat", "skip"])
    );
    assert_eq!(fields_at(FileKind::Curve, &["points"]), ["time", "value"]);
}

#[test]
fn the_curve_kinds_offer_all_four_flavours() {
    // decision 608, the user's ruling (NOT compared with the TypeScript until its fix merges): lfo, custom, adsr, onoff.
    assert_eq!(
        values_at(FileKind::Curve, &[], "type"),
        ["lfo", "custom", "adsr", "onoff"]
    );
    let f = fields_at(FileKind::Curve, &[]);
    assert!(
        ["type", "conditions", "points", "attack", "frequency", "polarity"]
            .iter()
            .all(|k| f.contains(k))
    );
    assert_eq!(values_at(FileKind::Curve, &[], "polarity"), ["unipolar", "bipolar"]);
    assert_eq!(
        values_at(FileKind::Curve, &[], "shape"),
        ["sine", "triangle", "saw", "square"]
    );
}

#[test]
fn hostile_paths_answer_in_time() {
    // hostile table (fields): a key path of 1,000,000 segments, a path of 1 MB, `\` and `..` in a path.
    let t = Instant::now();
    let deep: Vec<&str> = vec!["steps"; 1_000_000];
    assert!(fields_at(FileKind::Chain, &deep).is_empty());
    assert!(values_at(FileKind::Module, &deep, "type").is_empty());
    let long = format!("{}clips/x", "chains/".repeat(150_000));
    assert_eq!(file_kind_for(&long), Some(FileKind::Clip));
    let broken = format!("{}\n/graph.k7x", "chains/".repeat(150_000));
    assert_eq!(file_kind_for(&broken), None);
    assert_eq!(file_kind_for("..\\..\\modules\\x"), Some(FileKind::Module));
    assert_eq!(file_kind_for("../chains/graph.k7x"), Some(FileKind::Chains));
    assert!(t.elapsed() < bound(), "took {:?}", t.elapsed());
}
