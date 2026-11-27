// buffer_intake.rs — the buffer's reads and writes, the names a drop and a take get, and the rack builder's rules, with their TypeScript tests' inputs.
// A reader would get wrong that the page folds the accents: a name reaches `dropped_clip` already folded, as the page's `normalize` leaves it.

use k7x::edit::{Document, Value, parse};
use song_edit::buffer_text::*;
use song_edit::intake::*;
use song_edit::kit::*;

const G: ClipGrid = ClipGrid {
    tempo: 110.0,
    num: 4.0,
    den: 4.0,
};
// 110 BPM: one bar is 240/110 s, so "3:1" (8 beats) is 8*60/110 s.
const BAR: f64 = 240.0 / 110.0;

const FILE: &str =
    "// the break\nsource(\"clips/break.wav\")\nwarp({ algorithm: \"slice\", markers: [[0, 0], [4, 1.9]] })\n";

fn get(text: &str, key: &str) -> Option<Value> {
    match parse(text).ok()? {
        Value::Map(m) => m.into_iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn set_window(text: &str, from: f64, to: Option<f64>) -> String {
    let mut doc = Document::new(text);
    if !doc.errors().is_empty() {
        return text.to_owned();
    }
    set_clip_window(&mut doc, from, to, G).unwrap();
    doc.text().to_owned()
}

fn band(text: &str, editable: bool) -> Option<WindowBand> {
    clip_window_band(text, G, editable)
}

#[test]
fn a_window_written_in_seconds_or_as_a_timecode_is_read() {
    assert_eq!(band(FILE, false), None);
    let b = band(&format!("{FILE}from(1.32)\nto(4.36)\n"), false).unwrap();
    assert_eq!((b.from, b.to, b.hollow), (1.32, Some(4.36), false));
    let b = band(&format!("{FILE}to(\"3:1\")\n"), false).unwrap();
    assert_eq!(b.from, 0.0);
    assert!((b.to.unwrap() - 2.0 * BAR).abs() < 1e-9);
    // An unwritten `to` is the file's end, which only the view can place.
    assert_eq!(band(&format!("{FILE}from(1.5)\n"), false).unwrap().to, None);
}

#[test]
fn only_the_root_states_the_window_and_a_broken_text_draws_nothing() {
    let nested = "source(\"a.wav\")\nslices({ x: { from: 9, to: 10 } })\n";
    assert_eq!(scan_clip_window(nested), (None, None));
    assert_eq!(band(nested, false), None);
    assert_eq!(band(&format!("{FILE}to(\"3:\n"), false), None);
    assert_eq!(band(&format!("{FILE}to(\"3:\")\n"), false), None);
}

#[test]
fn an_untrimmed_clip_offers_hollow_edges_when_the_buffer_can_be_edited() {
    let b = band(FILE, true).unwrap();
    assert_eq!((b.from, b.to, b.hollow), (0.0, None, true));
    assert!(!band(&format!("{FILE}from(1)\n"), true).unwrap().hollow);
}

#[test]
fn a_dragged_edge_keeps_the_way_it_was_spelled() {
    let out = set_window(&format!("{FILE}from(1.32)\nto(4.36)\n"), 2.0, Some(5.0));
    assert_eq!(
        (get(&out, "from"), get(&out, "to")),
        (Some(Value::Num(2.0)), Some(Value::Num(5.0)))
    );
    // Pinned to a bar and stays pinned to one.
    let out = set_window(&format!("{FILE}to(\"3:1\")\n"), 0.0, Some(3.0 * BAR));
    assert_eq!(get(&out, "to"), Some(Value::Str("4:1".into())));
    // A timecode edge that did not move round-trips.
    let before = format!("{FILE}from(\"2:1\")\nto(\"3:1\")\n");
    let b = band(&before, false).unwrap();
    assert_eq!(set_window(&before, b.from, b.to), before);
}

#[test]
fn an_edge_that_was_not_there_is_written_in_seconds_and_a_start_leaves_the_key_out() {
    let out = set_window(FILE, 1.5, Some(4.0));
    assert!(out.contains("\nfrom(1.5)\n"));
    let out = set_window(&format!("{FILE}from(1.32)\nto(4.36)\n"), 0.0, Some(4.36));
    assert_eq!((get(&out, "from"), get(&out, "to")), (None, Some(Value::Num(4.36))));
    let out = set_window(&format!("{FILE}from(1)\nto(4)\n"), 2.0, None);
    assert_eq!((get(&out, "from"), get(&out, "to")), (Some(Value::Num(2.0)), None));
}

#[test]
fn a_write_touches_nothing_else_and_keeps_a_comment() {
    let out = set_window(&format!("{FILE}from(1.32)\nto(4.36)\n"), 2.0, Some(5.0));
    assert!(out.starts_with(FILE));
    let out = set_window(
        &format!("{FILE}from(1.32) // the count-in\nto(4.36)\n"),
        2.0,
        Some(4.36),
    );
    assert!(out.contains("from(2) // the count-in"));
    assert_eq!(
        get(&set_window(FILE, 0.0, Some(1.0 / 3.0)), "to"),
        Some(Value::Num(0.3333))
    );
}

#[test]
fn a_hollow_band_released_unmoved_writes_nothing() {
    let b = band(FILE, true).unwrap();
    assert_eq!(set_window(FILE, b.from, b.to), FILE);
    let out = set_window(FILE, 0.0, Some(2.5));
    assert_eq!((get(&out, "to"), get(&out, "from")), (Some(Value::Num(2.5)), None));
}

const SLICE: &str = "// a break, sliced\nrack({\n  rack_type: \"slice\",\n  source: \"break.wav\",\n  symbols: {\n    BD: { note: 36, from: 0, to: 0.5 },\n    SN: { note: 38, from: 0.5, to: 1.25 },\n  },\n})\n";

fn at(text: &str, r: (usize, usize)) -> String {
    text.chars().skip(r.0).take(r.1 - r.0).collect()
}

#[test]
fn each_slice_symbol_has_its_window_and_spans_that_address_exactly_its_numbers() {
    let out = scan_slices(SLICE).unwrap();
    let got: Vec<_> = out.iter().map(|s| (s.symbol.as_str(), s.from, s.to)).collect();
    assert_eq!(got, vec![("BD", 0.0, 0.5), ("SN", 0.5, 1.25)]);
    assert_eq!(at(SLICE, out[0].from_range), "0");
    assert_eq!(at(SLICE, out[0].to_range), "0.5");
    assert_eq!(at(SLICE, out[1].to_range), "1.25");
    assert_eq!(at(SLICE, out[0].key_range), "BD");
    let quoted = SLICE.replace("SN:", "\"S N\":");
    assert_eq!(at(&quoted, scan_slices(&quoted).unwrap()[1].key_range), "S N");
}

#[test]
fn no_slices_for_a_samples_rack_a_module_with_no_rack_or_a_broken_file() {
    assert!(scan_slices("rack({ rack_type: \"samples\", symbols: { BD: { note: 36, clip: \"kick\" } } })\n").is_none());
    assert!(scan_slices("plugin({ type: \"internal\", name: \"4osc\" })\n").is_none());
    assert!(scan_slices("rack({ rack_type: \"slice\", symbols: { BD: { from:").is_none());
}

fn slice_window(text: &str, symbol: &str, from: f64, to: f64) -> Option<String> {
    let mut doc = Document::new(text);
    set_slice_window(&mut doc, symbol, from, to)
        .unwrap()
        .then(|| doc.text().to_owned())
}

#[test]
fn a_slice_drag_rewrites_the_two_numbers_and_nothing_else() {
    let out = slice_window(SLICE, "SN", 0.5374, 1.0).unwrap();
    assert!(out.contains("SN: { note: 38, from: 0.5374, to: 1 },"));
    assert!(out.contains("BD: { note: 36, from: 0, to: 0.5 },"));
    assert!(out.contains("// a break, sliced"));
    let before: Vec<_> = SLICE.lines().collect();
    assert_eq!(out.lines().zip(&before).filter(|(a, b)| a != *b).count(), 1);
    // Float noise from a drag never lands in the file.
    let out = slice_window(SLICE, "BD", 0.123456789, 0.5).unwrap();
    assert!(out.contains("from: 0.1235"));
    assert_eq!(slice_window(SLICE, "HH", 0.0, 1.0), None);
    assert_eq!(slice_window("extends(\"x\")\n", "BD", 0.0, 1.0), None);
}

#[test]
fn the_reads_parse_only_when_a_read_could_answer() {
    let r = buffer_reads("name(\"lead\")\nparams({ gain: 0.5 })\n", "nodes/x.k7x");
    assert_eq!((r.slices, r.io, r.channels), (false, None, None));
    assert_eq!(buffer_reads("version(2)\naudio(true)\n", "nodes/x.k7x").io, None);
    let r = buffer_reads(
        &format!(
            "extends(\"@k7-core/kit\")\n{}",
            "rack({ rack_type: \"slice\", source: \"b.wav\", symbols: { BD: { note: 36, from: 0, to: 0.5 } } })\n"
        ),
        "n.k7x",
    );
    assert!(r.slices);
    assert!(!buffer_reads("rack({ rack_type: \"slice\", symbols: [\n", "n.k7x").slices);
}

#[test]
fn an_inputs_io_and_channels_come_from_one_parse() {
    let r = buffer_reads("extends(\"@meta/IN\")\nchannel([3, 4])\n", "nodes/x.k7x");
    assert_eq!(
        (r.io.as_deref(), r.channels),
        (Some("in"), Some(Channels::List(vec![3.0, 4.0])))
    );
    assert_eq!(
        buffer_reads("io(\"in\")\nchannel(\"Guitare\")\n", "x.k7x").channels,
        Some(Channels::Name("Guitare".into()))
    );
    // A path alone can make a file an input.
    let r = buffer_reads("channel([1])\n", "_modules/io/IN/in-1.k7x");
    assert_eq!(r.io.as_deref(), Some("in"));
    assert_eq!(buffer_reads("io(\"out\")\nchannel([1])\n", "x.k7x").channels, None);
    assert_eq!(
        buffer_reads("io(\"in\")\nchannel([0, 2.5, \"3\", -1])\n", "x.k7x").channels,
        Some(Channels::List(vec![3.0]))
    );
}

fn names(v: &[&str]) -> Vec<(String, String)> {
    v.iter().map(|n| ((*n).to_owned(), (*n).to_owned())).collect()
}

fn lanes() -> Vec<DropLane> {
    [
        ("gtr", DropKind::Audio),
        ("bass", DropKind::Audio),
        ("main", DropKind::Video),
        ("pip", DropKind::Video),
    ]
    .into_iter()
    .map(|(id, kind)| DropLane { id: id.into(), kind })
    .collect()
}

#[test]
fn a_dropped_file_is_read_by_its_extension_and_picture_wins_the_one_they_share() {
    assert_eq!(dropped_kind("kick.wav"), Some(DropKind::Audio));
    assert_eq!(dropped_kind("take.MP3"), Some(DropKind::Audio));
    assert_eq!(dropped_kind("shot.mov"), Some(DropKind::Video));
    assert_eq!(dropped_kind("clip.webm"), Some(DropKind::Video));
    assert_eq!(dropped_kind("still.PNG"), Some(DropKind::Video));
    assert_eq!(dropped_kind("subs.srt"), Some(DropKind::Video));
    assert_eq!(dropped_kind("notes.txt"), None);
    assert_eq!(dropped_kind("README"), None);
}

#[test]
fn a_clip_keeps_the_files_own_name_and_folds_what_a_path_would_argue_about() {
    let c = dropped_clip("Prise été n°3.wav", "Prise ete n°3.wav").unwrap();
    assert_eq!(
        (c.id.as_str(), c.upload.as_str(), c.kind),
        ("Prise-ete-n-3", "Prise-ete-n-3.wav", DropKind::Audio)
    );
    assert_eq!(
        dropped_clip("takes/sub/kick.WAV", "takes/sub/kick.WAV").unwrap().upload,
        "kick.wav"
    );
    assert_eq!(dropped_clip("---.wav", "---.wav").unwrap().id, "clip");
    assert_eq!(clip_decl("kick.wav").unwrap(), "source(\"clips/kick.wav\")\n");
    assert_eq!(clip_decl("a \"b\".wav").unwrap(), "source(\"clips/a \\\"b\\\".wav\")\n");
}

#[test]
fn a_drop_names_two_files_of_one_id_twice_and_walks_down_the_lanes_of_its_kind() {
    let p = plan_drop(&names(&["Kick 1.wav", "Kick-1.wav"]), &lanes(), "bass");
    let got: Vec<_> = p.declare.iter().map(|c| (c.id.as_str(), c.upload.as_str())).collect();
    assert_eq!(got, vec![("Kick-1", "Kick-1.wav"), ("Kick-1-2", "Kick-1-2.wav")]);
    let place = |names_: &[&str], at: &str| -> Vec<(String, String)> {
        plan_drop(&names(names_), &lanes(), at)
            .place
            .into_iter()
            .map(|p| (p.id, p.lane))
            .collect()
    };
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert_eq!(place(&["kick.wav"], "bass"), vec![pair("kick", "bass")]);
    assert_eq!(
        place(&["a.wav", "b.wav"], "gtr"),
        vec![pair("a", "gtr"), pair("b", "bass")]
    );
    // What runs out of lanes is left, but still declared.
    let p = plan_drop(&names(&["a.wav", "b.wav", "c.wav"]), &lanes(), "bass");
    assert_eq!(p.place.len(), 1);
    assert_eq!(p.declare.len(), 3);
    assert_eq!(
        place(&["a.mp4", "b.mp4"], "main"),
        vec![pair("a", "main"), pair("b", "main")]
    );
    assert_eq!(place(&["a.mp4", "kick.wav"], "main"), vec![pair("a", "main")]);
    assert!(place(&["kick.wav"], "ghost").is_empty());
}

#[test]
fn a_file_of_the_other_kind_is_declared_unplaced_and_a_stranger_is_refused() {
    let p = plan_drop(&names(&["shot.mp4"]), &lanes(), "gtr");
    assert!(p.place.is_empty());
    assert_eq!((p.declare[0].id.as_str(), p.declare[0].kind), ("shot", DropKind::Video));
    let p = plan_drop(&names(&["dir/notes.txt", "kick.wav"]), &lanes(), "gtr");
    assert_eq!(p.refused, vec!["notes.txt".to_owned()]);
    assert_eq!(p.declare.len(), 1);
}

#[test]
fn a_declaration_and_its_candidate_ids() {
    assert_eq!(
        declared_source("source(\"clips/kick.wav\")\n").as_deref(),
        Some("clips/kick.wav")
    );
    assert_eq!(
        declared_source("trim(1)\nsource('clips/a b.wav')\n").as_deref(),
        Some("clips/a b.wav")
    );
    assert_eq!(declared_source("gain(-3)\n"), None);
    assert_eq!(declared_source("source(\"clips/kick.wav\"\n"), None);
    assert_eq!(candidate_ids("kick", 3), vec!["kick", "kick-2", "kick-3"]);
}

#[test]
fn a_takes_container_name_id_and_line() {
    assert_eq!(ext_for_mime(Some("video/webm;codecs=vp9")), ".webm");
    assert_eq!(ext_for_mime(Some("video/webm")), ".webm");
    assert_eq!(ext_for_mime(Some("video/mp4")), ".mp4");
    assert_eq!(ext_for_mime(None), ".webm");
    assert_eq!(take_id("main", "260814T194357"), "main_260814T194357");
    assert_eq!(take_id("A roll/2", "S"), "A-roll-2_S");
    assert_eq!(take_id("...", "S"), "lane_S");
    assert_eq!(file_stamp(2026, 7, 9, 4, 5, 6), "260809T040506");
    assert_eq!(
        place_line("main", "main_S", 12.3456, 4.5),
        "picture add main main_S 12.346 4.5"
    );
    assert_eq!(place_line("main", "x", -1.0, -2.0), "picture add main x 0 0");
    assert_eq!(place_line("main", "x", f64::NAN, 1.0), "picture add main x NaN 1");
}

fn pad(symbol: &str, file: &str, note: f64) -> RackPad {
    RackPad {
        symbol: symbol.into(),
        file_name: file.into(),
        note,
    }
}

fn said(issues: &[RackIssue]) -> Vec<&str> {
    issues.iter().map(|i| i.key).collect()
}

#[test]
fn a_pad_fires_the_note_the_drum_machine_reading_says() {
    assert_eq!(pad_note(3, 0), 36.0);
    assert_eq!(pad_note(3, 3), 39.0);
    assert_eq!(pad_note(2, 0), 40.0);
    assert_eq!(pad_note(0, 0), 48.0);
    assert_eq!(pad_note(0, 3), 51.0);
    let notes: Vec<f64> = cells_in_note_order().into_iter().map(|(r, c)| pad_note(r, c)).collect();
    assert_eq!(notes, (0..16).map(|i| 36.0 + f64::from(i)).collect::<Vec<_>>());
}

#[test]
fn a_default_symbol_is_the_stem_squeezed_to_the_identifier_charset() {
    assert_eq!(default_symbol("kick.wav"), "KICK");
    assert_eq!(default_symbol("Snare Tight 01.wav"), "SNARE_TIGHT_01");
    assert_eq!(default_symbol("hi-hat.wav"), "HI-HAT");
    assert_eq!(default_symbol("...wav"), "PAD");
}

#[test]
fn what_stops_a_create() {
    let ok = [pad("BD", "BD.wav", 36.0), pad("SN", "sn.wav", 37.0)];
    assert!(rack_issues("drums", &ok, &[], true).is_empty());
    assert!(!rack_issues("", &ok, &[], true).is_empty());
    assert!(!rack_issues("a b", &ok, &[], true).is_empty());
    assert_eq!(
        said(&rack_issues("drums", &ok[..1], &["drums".to_owned()], true)),
        vec!["rb.exists"]
    );
    let dup = [pad("BD", "BD.wav", 36.0), pad("BD", "bd2.wav", 37.0)];
    assert_eq!(said(&rack_issues("d", &dup, &[], true)), vec!["rb.dupSymbol"]);
    let same = [pad("BD", "x.wav", 36.0), pad("SN", "x.wav", 37.0)];
    assert_eq!(said(&rack_issues("d", &same, &[], true)), vec!["rb.samePad"]);
    assert_eq!(said(&rack_issues("d", &[], &[], true)), vec!["rb.noSample"]);
    assert_eq!(
        said(&rack_issues("d", &[pad("B D", "x.wav", 36.0)], &[], true)),
        vec!["rb.badSymbol"]
    );
    let none = rack_issues("d", &ok[..1], &[], false);
    assert_eq!(
        (none[0].key, none[0].params["module"].as_str()),
        ("rb.noRack", "instruments/k7-rack")
    );
}

#[test]
fn a_kit_extends_a_librarys_sampler_and_never_a_workspace_copy() {
    let refs = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        rack_module_ref(&refs(&["@drums/instruments/k7-rack", "@k7-core/instruments/k7-rack"])).as_deref(),
        Some("@k7-core/instruments/k7-rack")
    );
    assert_eq!(
        rack_module_ref(&refs(&["@drums/instruments/k7-rack"])).as_deref(),
        Some("@drums/instruments/k7-rack")
    );
    assert_eq!(rack_module_ref(&refs(&["@workspace/instruments/k7-rack"])), None);
    assert_eq!(rack_module_ref(&refs(&["@engine/instruments/k7-rack"])), None);
    assert_eq!(rack_module_ref(&refs(&["@k7-core/instruments/rack"])), None);
    assert_eq!(rack_module_ref(&refs(&["@a/b/instruments/k7-rack"])), None);
}

#[test]
fn the_kit_node_is_a_node_of_the_sampler_with_one_tap_per_pad() {
    let pads = [pad("BD", "kick.wav", 36.0), pad("SN", "snare 01.wav", 37.0)];
    let rack = "@k7-core/instruments/k7-rack";
    let text = build_kit_node("drums", &pads, &KitDestination::Project(ProjectWord::Project), rack).unwrap();
    assert!(text.starts_with("// drums — a kit"));
    assert!(text.contains("\n    BD: { note: 36, sample: "));
    assert_eq!(get(&text, "extends"), Some(Value::Str(rack.into())));
    assert_eq!(get(&text, "engine"), Some(Value::Str("samples".into())));
    let Some(Value::Map(rack_map)) = get(&text, "rack") else {
        panic!("no rack")
    };
    let Value::Map(symbols) = &rack_map[0].1 else {
        panic!("no symbols")
    };
    assert_eq!(
        symbols.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        vec!["BD", "SN"]
    );
    assert!(text.contains("@project/drums/snare 01.wav"));
    let lib = build_kit_node(
        "drums",
        &pads,
        &KitDestination::Library {
            library: "k7-core".into(),
        },
        rack,
    )
    .unwrap();
    assert!(lib.contains("@k7-core/drums/kick.wav"));
}

#[test]
fn a_kits_samples_are_uploaded_into_the_samples_folder_its_name_points_at() {
    let project = KitDestination::Project(ProjectWord::Project);
    assert_eq!(
        kit_upload_dir(&project, "drums", "songs/first.k7"),
        "songs/first.k7/samples/drums"
    );
    assert_eq!(kit_upload_dir(&project, "drums", "."), "samples/drums");
    assert_eq!(
        kit_upload_dir(
            &KitDestination::Library {
                library: "k7-core".into()
            },
            "drums",
            "songs/first.k7"
        ),
        "_libraries/k7-core/samples/drums"
    );
}
