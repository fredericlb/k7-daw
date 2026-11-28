// views_fields_order.rs — the ORDER the completions offer fields in, which views_fields.rs compares as sets.
// A reader would get wrong that the order is cosmetic: the editor lists a field where the schema puts it, a lane's own last.

use song::views::fields::{FileKind, fields_at, file_kind_for, values_at};

#[test]
fn the_module_fields_come_in_the_order_the_schema_declares_them() {
    assert_eq!(
        fields_at(FileKind::Module, &[]),
        [
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
        ]
    );
}

#[test]
fn a_nested_struct_lists_its_fields_in_its_own_order() {
    assert_eq!(
        fields_at(FileKind::Module, &["ui"]),
        ["cols", "rows", "widgets", "tabs", "vertical"]
    );
    assert_eq!(fields_at(FileKind::Module, &["ui", "tabs", "0"]), ["name", "widgets"]);
    assert_eq!(
        fields_at(FileKind::Module, &["rack", "symbols", "BD"]),
        ["note", "sample", "clip", "from", "to", "mute"]
    );
    assert!(fields_at(FileKind::Module, &["params"]).is_empty());
    assert!(fields_at(FileKind::Module, &["plugin"]).is_empty());
}

#[test]
fn a_placement_lists_its_words_in_order() {
    assert_eq!(
        fields_at(FileKind::Module, &["events"]),
        [
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
            "pitch",
        ]
    );
}

#[test]
fn the_closed_sets_come_in_order_and_a_lane_file_is_a_module_file() {
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
    assert_eq!(file_kind_for("nodes/melody.k7x"), Some(FileKind::Module));
    assert_eq!(file_kind_for("tracks/melody.k7x"), Some(FileKind::Module));
}

#[test]
fn the_chains_files_sections_and_the_curve_types_come_in_order() {
    assert_eq!(
        fields_at(FileKind::Chains, &[]),
        ["modules", "bypassed", "bus_only", "tap_feeds", "positions"]
    );
    assert!(fields_at(FileKind::Chains, &["modules"]).is_empty());
    assert_eq!(
        values_at(FileKind::Curve, &[], "type"),
        ["lfo", "custom", "adsr", "onoff"]
    );
}
