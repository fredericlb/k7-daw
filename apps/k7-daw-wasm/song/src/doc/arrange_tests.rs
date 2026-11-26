// arrange_tests.rs — the cases of song-schema's trackKind.test.ts and trackColor.test.ts, which the TypeScript twin was held to.
// A reader would get wrong that each test names a TypeScript describe block: the Rust is the oracle now, these are its proofs.

use super::arrange::{
    TRACK_PREFIXES, TrackFamily, order_tracks, parse_track_name, sort_tracks_by_kind, track_kind, track_kind_rank,
    track_label,
};
use super::category::{TrackCategory, track_category};

fn prefix(name: &str) -> Option<&'static str> {
    parse_track_name(name).prefix.map(|(p, _)| p)
}

fn sorted(names: &[&str]) -> Vec<String> {
    sort_tracks_by_kind(names)
        .into_iter()
        .map(|i| names[i].to_owned())
        .collect()
}

fn ordered(names: &[&str], stated: &[&str]) -> Vec<String> {
    order_tracks(names, stated).into_iter().map(|i| names[i].to_owned()).collect()
}

#[test]
fn a_prefix_is_the_two_letter_code_before_the_first_dot_in_any_case() {
    assert_eq!(prefix("ge.lead"), Some("ge"));
    assert_eq!(prefix("da.kit"), Some("da"));
    assert_eq!(prefix("GE.Lead"), Some("ge"));
    assert_eq!(prefix("Sx.Solo"), Some("sx"));
    assert_eq!(prefix("vo.lead.double"), Some("vo"));
}

#[test]
fn a_name_without_a_usable_code_has_no_prefix() {
    for name in ["bass_3", ".hidden", "", "zz.what", "guitar.lead"] {
        assert_eq!(prefix(name), None, "{name}");
    }
}

#[test]
fn a_kind_is_its_family_and_its_label() {
    assert_eq!(track_kind("ge.lead").map(|(p, f, l)| (p, f.name(), l)), Some(("ge", "guitar", "guitare électrique")));
    assert_eq!(track_kind("da.kit").map(|k| k.1), Some(TrackFamily::Drums));
    assert_eq!(track_kind("ds.808").map(|k| k.1), Some(TrackFamily::Drums));
    for p in ["be", "ba", "bs"] {
        assert_eq!(track_kind(&format!("{p}.x")).map(|k| k.1), Some(TrackFamily::Bass), "{p}");
    }
    assert_eq!(track_kind("5.da.kit").map(|k| k.1), Some(TrackFamily::Drums));
    assert_eq!(track_kind("lead"), None);
    assert_eq!(TRACK_PREFIXES.len(), 12);
}

#[test]
fn the_families_lay_out_in_mix_order() {
    let names = ["sx.solo", "sy.pad", "pi.rhodes", "ga.rythm", "be.main", "da.kit", "ch.alto", "vo.lead"];
    assert_eq!(
        sorted(&names),
        ["vo.lead", "ch.alto", "da.kit", "be.main", "ga.rythm", "pi.rhodes", "sy.pad", "sx.solo"]
    );
}

#[test]
fn unprefixed_tracks_go_last_in_the_order_they_came_and_a_family_keeps_its_own_order() {
    assert_eq!(sorted(&["zoo", "vo.lead", "alpha", "da.kit"]), ["vo.lead", "da.kit", "zoo", "alpha"]);
    assert_eq!(sorted(&["be.z", "ba.a", "bs.m"]), ["be.z", "ba.a", "bs.m"]);
}

#[test]
fn an_unprefixed_name_is_placed_by_what_it_says() {
    assert_eq!(
        sorted(&["drums", "bass_3", "audio_6", "vox", "track-1"]),
        ["vox", "drums", "bass_3", "audio_6", "track-1"]
    );
    assert!(track_kind_rank("zz.x") > track_kind_rank("sx.x"));
}

#[test]
fn a_name_is_taken_apart_into_position_prefix_and_label() {
    let p = parse_track_name("3.ge.lead");
    assert_eq!((p.position, p.prefix.map(|x| x.0), p.label), (Some(3.0), Some("ge"), "lead"));
    let p = parse_track_name("ge.lead");
    assert_eq!((p.position, p.prefix.map(|x| x.0), p.label), (None, Some("ge"), "lead"));
    let p = parse_track_name("2.stems");
    assert_eq!((p.position, p.prefix.map(|x| x.0), p.label), (Some(2.0), None, "stems"));
    assert_eq!(parse_track_name("1.vo.lead.double").label, "lead.double");
    let p = parse_track_name("12");
    assert_eq!((p.position, p.prefix.map(|x| x.0), p.label), (None, None, "12"));
    let p = parse_track_name("zz.what");
    assert_eq!((p.position, p.prefix.map(|x| x.0), p.label), (None, None, "zz.what"));
    assert_eq!(parse_track_name("5.da.kit").position, Some(5.0));
}

#[test]
fn a_pinned_position_puts_a_track_in_its_slot() {
    assert_eq!(sorted(&["vo.lead", "da.kit", "2.sx.solo"]), ["vo.lead", "2.sx.solo", "da.kit"]);
    assert_eq!(
        sorted(&["sy.pad", "1.zz.slate", "vo.lead", "4.ga.rythm"]),
        ["1.zz.slate", "vo.lead", "sy.pad", "4.ga.rythm"]
    );
}

#[test]
fn the_loser_of_a_duplicated_number_moves_down_and_a_number_past_the_end_lands_last() {
    assert_eq!(sorted(&["2.a", "2.b", "vo.lead"]), ["vo.lead", "2.a", "2.b"]);
    assert_eq!(sorted(&["99.outro", "vo.lead", "da.kit"]), ["vo.lead", "da.kit", "99.outro"]);
}

#[test]
fn zero_and_one_both_mean_first_and_no_pin_drops_a_track() {
    assert_eq!(sorted(&["vo.lead", "0.slate"])[0], "0.slate");
    assert_eq!(sorted(&["vo.lead", "1.slate"])[0], "1.slate");
    let names = ["9.a", "9.b", "9.c", "1.d", "vo.e", "f"];
    let mut out = sorted(&names);
    out.sort();
    let mut all: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
    all.sort();
    assert_eq!(out, all);
}

#[test]
fn a_label_strips_the_metadata_a_view_shows_another_way() {
    for (name, label) in [
        ("3.ge.lead", "lead"),
        ("ge.lead", "lead"),
        ("2.stems", "stems"),
        ("lead", "lead"),
        ("1.vo.lead.double", "lead.double"),
        ("bass_3", "bass_3"),
        ("zz.what", "zz.what"),
        ("ge.", "ge."),
    ] {
        assert_eq!(track_label(name), label, "{name}");
    }
}

#[test]
fn a_stated_order_beats_the_names_outright() {
    let names = ["ge.lead", "vo.main", "da.kit"];
    assert_eq!(ordered(&names, &[]), ["vo.main", "da.kit", "ge.lead"]);
    assert_eq!(ordered(&names, &["ge.lead", "da.kit", "vo.main"]), ["ge.lead", "da.kit", "vo.main"]);
    assert_eq!(ordered(&names, &["ge.lead"]), ["ge.lead", "vo.main", "da.kit"]);
    assert_eq!(ordered(&names, &["gone", "da.kit"]), ["da.kit", "vo.main", "ge.lead"]);
}

fn category(name: &str) -> TrackCategory {
    track_category::<&str>(name, &[])
}

#[test]
fn a_category_reads_a_declared_prefix_first() {
    use TrackCategory::*;
    for (name, want) in [
        ("vo.couz", Vox),
        ("ch.back", Vox),
        ("da.main", Drums),
        ("bs.acid", Bass),
        ("ge.lead", Guitar),
        ("pi.wurli", Keys),
        ("sy.jun", Synth),
        ("sx.solo", Other),
    ] {
        assert_eq!(category(name), want, "{name}");
    }
}

#[test]
fn a_category_reads_what_an_unprefixed_name_says() {
    use TrackCategory::*;
    for (name, want) in [
        ("acoustic_drums_kit_ludwig_2_processed", Drums),
        ("legend_505_kit", Drums),
        ("basse", Bass),
        ("vox", Vox),
        ("folk", Guitar),
        ("grand_piano_2", Keys),
        ("polysynth-copy", Synth),
        ("track-1", Other),
    ] {
        assert_eq!(category(name), want, "{name}");
    }
}

#[test]
fn a_category_falls_back_on_what_the_instrument_says_and_only_then_on_other() {
    assert_eq!(track_category("audio_7", &["mu-tides"]), TrackCategory::Synth);
    assert_eq!(track_category("michel", &["@k7-core/instruments/keys/electric-keys"]), TrackCategory::Keys);
    assert_eq!(track_category("audio_7", &["mixer"]), TrackCategory::Other);
}
