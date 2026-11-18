// doc_arrange.rs — the pure helpers of the document: track order, category, colour, ownMix, highlight, the group fold, the finite walk.
// A reader would get wrong that the expected answers are the TypeScript's, read off its sources: a hostile case asserts the answer too.

use crate::doc_support;

use doc_support::{CATEGORIES, HELD, LEVELS, MIXES, ORDERS, PARSED, RANKS, SHADES};
use song::doc::{
    BlendShape, GroupDoc, LevelSide, MixSide, PluginDoc, ProjectDoc, TrackCategory, TrackDoc, auto_colors,
    category_of_text, category_shade, held_params, highlighted, order_tracks, own_level_side, own_mix_side,
    parse_track_name, sort_tracks_by_kind, track_category, track_kind_rank, with_group_mix,
};
use song::modules::ModuleKind;
use std::time::{Duration, Instant};

fn category(name: &str) -> TrackCategory {
    match name {
        "vox" => TrackCategory::Vox,
        "drums" => TrackCategory::Drums,
        "bass" => TrackCategory::Bass,
        "guitar" => TrackCategory::Guitar,
        "keys" => TrackCategory::Keys,
        "synth" => TrackCategory::Synth,
        _ => TrackCategory::Other,
    }
}

fn is_permutation(order: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    order.len() == n && order.iter().all(|&i| i < n && !std::mem::replace(&mut seen[i], true))
}

fn within(t: Instant, what: &str) {
    assert!(
        t.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{what}: {:?}",
        t.elapsed()
    );
}

// ts: trackKind.ts orderTracks over fixed lists; decision 325.
#[test]
fn the_order_of_lanes_is_the_typescripts_stated_or_by_kind() {
    for (names, stated, expected) in ORDERS {
        let order = match stated {
            Some(s) => order_tracks(names, s),
            None => order_tracks::<&str>(names, &[]),
        };
        let got: Vec<&str> = order.iter().map(|&i| names[i]).collect();
        assert_eq!(&got, expected, "{names:?} stated {stated:?}");
    }
}

// ts: trackKind.ts parseTrackName; decisions 325 and 357.
#[test]
fn a_name_is_taken_apart_as_the_typescript_does_and_a_prototype_key_is_a_plain_name() {
    for (name, position, prefix, label) in PARSED {
        let p = parse_track_name(name);
        assert_eq!(
            (p.position, p.prefix.map(|x| x.0), p.label),
            (*position, *prefix, *label),
            "{name:?}"
        );
    }
    for name in [
        "constructor.x",
        "__proto__.x",
        "toString.x",
        "hasOwnProperty.x",
        "valueOf.x",
    ] {
        let p = parse_track_name(name);
        assert_eq!((p.prefix, p.label), (None, name), "{name}");
    }
}

// ts: trackKind.ts trackKindRank and trackCategory; decision 326.
#[test]
fn a_category_and_a_rank_are_the_typescripts() {
    for (name, expected) in CATEGORIES.iter().take(CATEGORIES.len() - 12) {
        assert_eq!(track_category::<&str>(name, &[]), category(expected), "{name:?}");
    }
    for (name, rank) in RANKS {
        assert_eq!(track_kind_rank(name), *rank, "{name:?}");
    }
    let tail = &CATEGORIES[CATEGORIES.len() - 12..];
    let hint = |n: &str| match n {
        "thing" => vec!["a Moog bass", "x"],
        "thing2" => vec!["nothing", "Rhodes piano"],
        _ => vec![],
    };
    for (name, expected) in tail {
        assert_eq!(track_category(name, &hint(name)), category(expected), "{name:?}");
    }
}

// no ts test: the words of trackCategory with their edges; decision 326.
#[test]
fn a_word_needs_its_ascii_edges_where_the_regex_asks_for_them() {
    let says = |t: &str| category_of_text(t);
    assert_eq!(says("hi-hat"), Some(TrackCategory::Drums));
    assert_eq!(says("whatever"), None);
    assert_eq!(says("x_hat_y"), Some(TrackCategory::Drums));
    assert_eq!(says("hatx"), None);
    assert_eq!(says("a.808.b"), Some(TrackCategory::Drums));
    assert_eq!(says("8080"), None);
    assert_eq!(says("Chœur"), None);
    assert_eq!(says("CHŒEUR"), Some(TrackCategory::Vox));
    assert_eq!(says("éhat"), Some(TrackCategory::Drums));
    assert_eq!(says("İkit"), Some(TrackCategory::Drums));
    assert_eq!(says("kitkit"), None);
    assert_eq!(says("kitkit kit"), Some(TrackCategory::Drums));
    assert_eq!(says("clavinet"), Some(TrackCategory::Keys));
    assert_eq!(says("reclav"), None);
    assert_eq!(says("mu-x"), Some(TrackCategory::Synth));
}

// ts: trackColor.ts categoryShade and autoColors; decision 327.
#[test]
fn a_shade_and_a_colour_are_the_typescripts() {
    for (c, i, n, hex) in SHADES {
        assert_eq!(category_shade(category(c), *i, *n), *hex, "{c} {i}/{n}");
    }
    let tracks = [
        ("a", TrackCategory::Drums),
        ("b", TrackCategory::Vox),
        ("c", TrackCategory::Drums),
        ("a", TrackCategory::Bass),
        ("d", TrackCategory::Drums),
    ];
    let got = auto_colors(&tracks);
    let want = [("a", "#935b34"), ("b", "#e85ea3"), ("c", "#dd322c"), ("d", "#e57361")];
    assert_eq!(got.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>(), want);
}

// ts: moduleTypes.ts ownLevelSide and ownMixSide; decision 314.
#[test]
fn the_sides_of_a_stage_param_follow_the_regexes_case_blind_and_ascii_only() {
    for (name, side) in LEVELS {
        let got = own_level_side(name).map(|s| if s == LevelSide::Input { "input" } else { "output" });
        assert_eq!(got, *side, "{name:?}");
    }
    for (name, side) in MIXES {
        let got = own_mix_side(name).map(|s| if s == MixSide::Wet { "wet" } else { "dry" });
        assert_eq!(got, *side, "{name:?}");
    }
}

// ts: ownMix.ts heldParams over shapes; decision 314.
#[test]
fn the_params_the_stage_holds_follow_the_modules_shape() {
    let shape = |s: &str| -> Option<BlendShape<'static>> {
        let b = |kind, category, inputs| Some(BlendShape { kind, category, inputs });
        match s {
            "null" => None,
            r#"{"category":"audio"}"# => b(None, Some("audio"), 0),
            r#"{"category":"audio","inputs":[1]}"# => b(None, Some("audio"), 1),
            r#"{"category":"audio","inputs":[1,2]}"# => b(None, Some("audio"), 2),
            r#"{"category":"instruments"}"# => b(None, Some("instruments"), 0),
            r#"{"kind":"io","category":"audio"}"# => b(Some(ModuleKind::Io), Some("audio"), 0),
            r#"{"kind":"doc"}"# => b(Some(ModuleKind::Doc), None, 0),
            r#"{"kind":"plugin","category":"audio"}"# => b(Some(ModuleKind::Plugin), Some("audio"), 0),
            r#"{"category":"midi"}"# => b(None, Some("midi"), 0),
            other => panic!("a shape the table did not name: {other}"),
        }
    };
    let keys = [
        "Input",
        "Output Gain",
        "Dry/Wet",
        "Mix",
        "Cutoff",
        "@mix",
        "out",
        "dry",
        "Volume",
    ];
    for (json, held) in HELD {
        assert_eq!(held_params(shape(json), keys), *held, "{json}");
    }
}

fn mixer(id: &str, out: Option<f64>) -> PluginDoc {
    let mut p = PluginDoc::new(id);
    p.plugin_type = "Mixer".to_owned();
    p.params.insert("Volume".to_owned(), -3.0);
    if let Some(v) = out {
        p.params.insert("@output-volume".to_owned(), v);
    }
    p
}

// ts: highlight.ts highlighted; decision 324.
#[test]
fn the_last_mixer_plays_ten_up_to_the_ceiling_and_a_lane_without_one_is_left() {
    let mut none = vec![PluginDoc::new("a:x")];
    highlighted(&mut none);
    assert_eq!(none, vec![PluginDoc::new("a:x")]);
    let mut two = vec![mixer("a:m1", None), PluginDoc::new("a:x"), mixer("a:m2", Some(1.5))];
    highlighted(&mut two);
    assert_eq!(two[0].params.get("@output-volume"), None);
    assert_eq!(two[2].params.get("@output-volume"), Some(&11.5));
    assert_eq!(two[2].params.keys().collect::<Vec<_>>(), ["Volume", "@output-volume"]);
    let mut first = vec![mixer("a:m", None)];
    highlighted(&mut first);
    assert_eq!(first[0].params.get("@output-volume"), Some(&10.0));
    let mut top = vec![mixer("a:m", Some(8.0))];
    highlighted(&mut top);
    assert_eq!(top[0].params.get("@output-volume"), Some(&12.0));
    let mut nan = vec![mixer("a:m", Some(f64::NAN))];
    highlighted(&mut nan);
    assert_eq!(nan[0].params.get("@output-volume"), Some(&10.0));
}

fn group(id: &str, mute: bool, solo: bool) -> GroupDoc {
    GroupDoc {
        id: id.to_owned(),
        name: id.to_owned(),
        color: String::new(),
        mute,
        solo,
    }
}

fn member(id: &str, group: &str, mute: bool, solo: bool) -> TrackDoc {
    let mut t = TrackDoc::new(id);
    (t.group, t.mute, t.solo) = (group.to_owned(), mute, solo);
    t
}

// ts: project.ts withGroupMix; decision 331.
#[test]
fn a_group_folds_its_mute_and_solo_onto_its_members_by_or() {
    let mut doc = ProjectDoc::empty();
    doc.groups = vec![
        group("g", true, false),
        group("h", false, true),
        group("quiet", false, false),
        group("g", false, true),
    ];
    doc.tracks = vec![
        member("a", "g", false, false),
        member("b", "h", true, false),
        member("c", "quiet", false, false),
        member("d", "", false, false),
        member("e", "missing", false, false),
    ];
    with_group_mix(&mut doc);
    let flags: Vec<(bool, bool)> = doc.tracks.iter().map(|t| (t.mute, t.solo)).collect();
    // The last `g` wins: solo, no mute.
    assert_eq!(
        flags,
        [
            (false, true),
            (true, true),
            (false, false),
            (false, false),
            (false, false)
        ]
    );
    let mut alone = ProjectDoc::empty();
    alone.tracks = vec![member("a", "g", false, false)];
    with_group_mix(&mut alone);
    assert!(!alone.tracks[0].mute);
}

// no ts test: the finite walk of decision 354 reaches every number the engine reads.
#[test]
fn a_number_that_is_not_finite_anywhere_makes_the_document_not_finite() {
    assert!(ProjectDoc::empty().finite());
    let mutate: [fn(&mut ProjectDoc); 7] = [
        |d| d.tempo = f64::NAN,
        |d| d.master_volume = f64::INFINITY,
        |d| {
            d.markers.push(song::Marker {
                tc: "1:1".into(),
                name: "m".into(),
                beat: f64::NAN,
            })
        },
        |d| {
            d.tracks.push({
                let mut t = TrackDoc::new("t");
                t.input_gain = f64::NAN;
                t
            })
        },
        |d| {
            d.tracks.push({
                let mut t = TrackDoc::new("t");
                t.sends.push(song::doc::SendDoc {
                    to: "b".into(),
                    level: f64::INFINITY,
                    enabled: true,
                });
                t
            })
        },
        |d| {
            d.tracks.push({
                let mut t = TrackDoc::new("t");
                t.automation.push(song::doc::AutomationDoc {
                    param: "p".into(),
                    plugin_id: None,
                    bipolar: None,
                    points: vec![[0.0, f64::NAN]],
                });
                t
            })
        },
        |d| {
            d.master_plugins.push({
                let mut p = PluginDoc::new("m:x");
                p.params.insert("k".into(), f64::NEG_INFINITY);
                p
            })
        },
    ];
    for (i, m) in mutate.iter().enumerate() {
        let mut d = ProjectDoc::empty();
        m(&mut d);
        assert!(!d.finite(), "case {i}");
    }
}

// no ts test: hostile names (decisions 325, 326); each answered within the hang bound with the answer asserted.
#[test]
fn a_hundred_thousand_pinned_lanes_are_placed_in_linear_time_and_every_one_is_kept() {
    let names: Vec<String> = (0..100_000).map(|i| format!("1.x{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let t = Instant::now();
    let order = sort_tracks_by_kind(&refs);
    within(t, "100,000 pinned lanes");
    assert!(is_permutation(&order, refs.len()));
    assert_eq!(order[..3], [0, 1, 2]);
    let stated: Vec<String> = (0..100_000).map(|i| format!("absent{i}")).collect();
    let t = Instant::now();
    let order = order_tracks(&refs, &stated);
    within(t, "100,000 stated names, all absent");
    assert_eq!(order, sort_tracks_by_kind(&refs));
}

// no ts test: the awkward names of the hostile table; decision 325.
#[test]
fn awkward_names_are_ordered_without_a_panic_and_a_listed_twice_name_takes_its_last_place() {
    let big = "9".repeat(400);
    let names = [
        "99999999999999999999.a",
        ".x",
        "x.",
        "..",
        "0.1.y",
        "z",
        &format!("{big}.q"),
        &"w".repeat(1 << 20),
    ];
    let order = sort_tracks_by_kind(&names);
    assert!(is_permutation(&order, names.len()));
    assert_eq!(parse_track_name(names[6]).position, Some(f64::INFINITY));
    let two = order_tracks(&["a", "b", "c"], &["c", "a", "c"]);
    assert_eq!(two, [0, 2, 1]);
}

// no ts test: a category word in a name of 5 MB is one linear pass per needle; decision 326.
#[test]
fn a_five_megabyte_name_is_read_in_a_pass_per_needle() {
    let name = "kikikitt".repeat(5 << 20 >> 3);
    let t = Instant::now();
    assert_eq!(category_of_text(&name), None);
    assert_eq!(category_of_text(&format!("{name} kit")), Some(TrackCategory::Drums));
    within(t, "a name of 5 MB");
    let t = Instant::now();
    assert_eq!(category_of_text(&"İ".repeat(1 << 20)), None);
    within(t, "a capital İ, a million times");
}

// no ts test: one walk over the plugins of a lit lane; decision 324.
#[test]
fn a_hundred_thousand_plugins_on_a_lit_lane_are_one_walk() {
    let mut plugins: Vec<PluginDoc> = (0..100_000).map(|i| PluginDoc::new(&format!("a:p{i}"))).collect();
    plugins[50_000] = mixer("a:m", None);
    let t = Instant::now();
    highlighted(&mut plugins);
    within(t, "100,000 plugins");
    assert_eq!(plugins[50_000].params.get("@output-volume"), Some(&10.0));
}
