// rules.rs — the small rules: the beat grid, a corrected tempo, loops, the crossfade law, sync, the scratch pace.
// A reader would get wrong whose cases these are: the TypeScript tests of beats, grid, loop and deck, with their inputs unchanged.

use indexmap::IndexMap;
use turntables::beats::{GridOf, Needles, align_seek, beat_phase};
use turntables::deck::{
    crossfade_levels, deck_gain, gain_to_db, nudged_tempos, platter_key, platter_rate, platters_of, scratch_delay,
    stem_source, synced_tempos, with_keylock,
};
use turntables::grid::corrected;
use turntables::loops::{
    DeckLoop, LoopEdges, beat_jump, beat_seconds, beats_between, can_loop, loop_edges, loop_params, loop_span, resized,
    snap_to_beat,
};
use turntables::rebuild::worth_a_rebuild;
use turntables::vocab::{DeckLoad, DeckTempo, DeckTempos};

fn tempo(natural: f64, target: f64) -> DeckTempo {
    DeckTempo {
        natural,
        seconds: 60.0,
        target,
        phase: None,
        keylock: None,
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn a_grid_is_found_from_onsets_that_sit_on_it() {
    let ts: Vec<f64> = (0..32).map(|i| 0.13 + f64::from(i) * 0.5).collect();
    let g = beat_phase(&ts, 120.0);
    near(g.phase, 0.13);
    assert!(g.strength > 0.999 && g.confident);
}

#[test]
fn a_grid_survives_a_slop_the_first_onset_would_not() {
    let ts: Vec<f64> = (0..64)
        .map(|i| 0.2 + f64::from(i) * 0.5 + if i % 2 == 0 { 0.02 } else { -0.02 })
        .collect();
    assert!((beat_phase(&ts, 120.0).phase - 0.2).abs() < 0.01);
}

#[test]
fn no_confidence_when_the_onsets_say_nothing_and_no_grid_without_a_tempo() {
    // Onsets spread evenly round the beat cancel out: a strength near zero.
    let ts: Vec<f64> = (0..50).map(|i| f64::from(i) * 0.01).collect();
    let g = beat_phase(&ts, 120.0);
    assert!(!g.confident && g.strength < 0.2);
    assert_eq!(beat_phase(&[], 120.0).phase, 0.0);
    assert!(!beat_phase(&[1.0, 2.0], 0.0).confident);
}

#[test]
fn a_phase_always_lands_inside_one_beat() {
    for start in [-3.3, 0.0, 7.77, 100.1] {
        let ts: Vec<f64> = (0..20).map(|i| start + f64::from(i) * 0.5).collect();
        let g = beat_phase(&ts, 120.0);
        assert!((0.0..0.5).contains(&g.phase), "{}", g.phase);
    }
}

fn grid(natural: f64, phase: Option<f64>) -> GridOf {
    GridOf { natural, phase }
}

#[test]
fn a_slave_is_put_on_the_masters_beat_by_the_shorter_way() {
    let m = grid(120.0, Some(0.1));
    let s = grid(120.0, Some(0.3));
    let to = align_seek(
        m,
        s,
        Needles {
            master: 10.1,
            slave: 20.0,
        },
    )
    .unwrap();
    // Master sits exactly on a beat; the slave's grid is 0.2 s late, so it moves back to its own beat.
    near((to - 0.3).rem_euclid(0.5), 0.0);
    assert!((to - 20.0).abs() <= 0.25 + 1e-9);
}

#[test]
fn a_seek_never_lands_before_the_record_starts_and_unknown_grids_say_nothing() {
    let m = grid(120.0, Some(0.0));
    let s = grid(120.0, Some(0.0));
    let to = align_seek(
        m,
        s,
        Needles {
            master: 0.4,
            slave: 0.0,
        },
    )
    .unwrap();
    assert!(to >= 0.0);
    assert!(
        align_seek(
            grid(120.0, None),
            s,
            Needles {
                master: 1.0,
                slave: 1.0
            }
        )
        .is_none()
    );
    assert!(
        align_seek(
            m,
            grid(0.0, Some(0.0)),
            Needles {
                master: 1.0,
                slave: 1.0
            }
        )
        .is_none()
    );
}

#[test]
fn a_correction_replaces_what_the_record_is_and_an_untouched_fader_follows() {
    let t = DeckTempo {
        phase: Some(0.2),
        ..tempo(174.0, 174.0)
    };
    let c = corrected(&t, Some(87.0));
    assert_eq!((c.natural, c.target, c.seconds, c.phase), (87.0, 87.0, 60.0, Some(0.2)));
    assert_eq!(corrected(&t, None), t);
    // A fader somebody already moved is a decision.
    let moved = tempo(174.0, 170.0);
    assert_eq!(corrected(&moved, Some(87.0)).target, 170.0);
}

#[test]
fn a_beat_is_the_records_own_and_zero_with_nothing_measured() {
    near(beat_seconds(Some(&tempo(120.0, 90.0))), 0.5);
    assert_eq!(beat_seconds(None), 0.0);
    assert!(can_loop(Some(&tempo(120.0, 120.0))));
    assert!(!can_loop(Some(&tempo(0.0, 0.0))));
    assert!(!can_loop(Some(&DeckTempo {
        seconds: 0.0,
        ..tempo(120.0, 120.0)
    })));
}

#[test]
fn snapping_goes_to_the_nearest_beat_and_never_before_the_start() {
    let t = DeckTempo {
        phase: Some(0.2),
        ..tempo(120.0, 120.0)
    };
    near(snap_to_beat(1.0, Some(&t)), 1.2);
    near(snap_to_beat(1.4, Some(&t)), 1.2);
    // Rounding half up, as the page's Math.round does.
    near(snap_to_beat(0.45, Some(&tempo(120.0, 120.0))), 0.5);
    assert_eq!(snap_to_beat(3.3, None), 3.3);
    let late = DeckTempo {
        phase: Some(0.4),
        ..tempo(120.0, 120.0)
    };
    assert!(snap_to_beat(0.0, Some(&late)) >= 0.0);
}

#[test]
fn a_loop_length_halves_doubles_and_stops_at_both_ends() {
    assert_eq!(resized(4.0, 2.0), 8.0);
    assert_eq!(resized(32.0, 2.0), 32.0);
    assert_eq!(resized(0.125, 0.5), 0.125);
    let t = tempo(120.0, 120.0);
    assert_eq!(beats_between(0.0, 4.2, Some(&t)), 8.0);
    assert_eq!(beats_between(0.0, 0.3, Some(&t)), 0.5);
    assert_eq!(beats_between(0.0, 1000.0, Some(&t)), 32.0);
    assert_eq!(beats_between(0.0, 1.0, None), 4.0);
}

#[test]
fn edges_are_fractions_of_the_record_clamped_into_it() {
    let t = DeckTempo {
        seconds: 100.0,
        ..tempo(120.0, 140.0)
    };
    let lp = DeckLoop {
        start: 10.0,
        beats: 8.0,
        roll: false,
    };
    // Eight beats at the RECORD's 120, not the fader's 140.
    assert_eq!(loop_edges(&lp, Some(&t)), Some(LoopEdges { start: 0.1, end: 0.14 }));
    let off = DeckLoop { start: 99.0, ..lp };
    assert_eq!(loop_edges(&off, Some(&t)).map(|e| e.end), Some(1.0));
    let none = DeckLoop { start: 100.0, ..lp };
    assert_eq!(loop_edges(&none, Some(&t)), None);
    assert_eq!(loop_edges(&lp, None), None);
}

#[test]
fn a_jump_moves_by_whole_beats_stays_inside_and_needs_a_grid() {
    let t = DeckTempo {
        seconds: 100.0,
        ..tempo(120.0, 120.0)
    };
    near(beat_jump(10.0, 4.0, Some(&t), false), 12.0);
    near(beat_jump(10.2, -4.0, Some(&t), true), 8.0);
    assert_eq!(beat_jump(1.0, -16.0, Some(&t), false), 0.0);
    assert_eq!(beat_jump(99.0, 16.0, Some(&t), false), 100.0);
    assert_eq!(beat_jump(5.0, 4.0, None, false), 5.0);
}

#[test]
fn the_params_go_in_the_order_the_platter_needs() {
    let e = LoopEdges { start: 0.1, end: 0.2 };
    let names = |v: Vec<(String, f64)>| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(names(loop_params(None, false, false)), ["LoopEnd"]);
    assert_eq!(
        names(loop_params(Some(e), true, false)),
        ["Roll", "LoopStart", "LoopEnd"]
    );
    assert_eq!(loop_params(Some(e), true, false)[0].1, 1.0);
    assert_eq!(names(loop_params(Some(e), false, true)), ["LoopEnd"]);
}

#[test]
fn a_loop_is_drawn_in_source_seconds_when_it_has_room() {
    let t = DeckTempo {
        seconds: 10.0,
        ..tempo(120.0, 120.0)
    };
    let lp = DeckLoop {
        start: 2.0,
        beats: 4.0,
        roll: false,
    };
    let span = loop_span(Some(&lp), Some(&t)).unwrap();
    assert_eq!((span.start, span.end), (2.0, 4.0));
    assert!(loop_span(Some(&DeckLoop { start: 10.0, ..lp }), Some(&t)).is_none());
    assert!(loop_span(None, Some(&t)).is_none());
}

#[test]
fn the_crossfade_is_equal_gain_and_clamps_rather_than_inverting() {
    for mix in [0.0, 0.25, 0.5, 0.75, 1.0] {
        near(deck_gain("deckA", mix), 1.0 - mix);
        near(deck_gain("deckB", mix), mix);
    }
    assert_eq!(deck_gain("deckA", 1.5), 0.0);
    assert_eq!(deck_gain("deckB", -0.5), 0.0);
}

#[test]
fn a_gain_is_decibels_with_silence_on_the_floor() {
    assert_eq!(gain_to_db(1.0), 0.0);
    assert_eq!(gain_to_db(0.0), -60.0);
    assert!((gain_to_db(0.5) + 6.02).abs() < 0.01);
    let ends = crossfade_levels(0.0);
    assert_eq!(
        (ends[0].id.as_str(), ends[0].db, ends[1].db),
        ("deckA:level", 0.0, -60.0)
    );
}

fn both() -> DeckTempos {
    IndexMap::from([
        ("deckA".to_owned(), tempo(120.0, 126.0)),
        ("deckB".to_owned(), tempo(90.0, 90.0)),
    ])
}

#[test]
fn sync_hands_one_decks_tempo_to_the_other_whichever_leads() {
    assert_eq!(synced_tempos(&both(), Some("deckA"))["deckB"].target, 126.0);
    assert_eq!(synced_tempos(&both(), Some("deckB"))["deckA"].target, 90.0);
    assert_eq!(synced_tempos(&both(), None), both());
}

#[test]
fn sync_needs_a_tempo_at_both_ends() {
    let mut t = both();
    t["deckA"] = tempo(0.0, 0.0);
    assert_eq!(synced_tempos(&t, Some("deckA"))["deckB"].target, 90.0);
    let mut t = both();
    t["deckB"] = tempo(0.0, 0.0);
    assert_eq!(synced_tempos(&t, Some("deckA"))["deckB"].target, 0.0);
}

#[test]
fn a_nudge_moves_the_target_alone_and_keylock_is_merged_in_per_deck() {
    let n = nudged_tempos(&both(), "deckA", 100.0);
    assert_eq!((n["deckA"].target, n["deckA"].natural), (100.0, 120.0));
    assert_eq!(nudged_tempos(&both(), "ghost", 1.0), both());
    let on = IndexMap::from([("deckA".to_owned(), true)]);
    let k = with_keylock(&both(), &on);
    assert_eq!((k["deckA"].keylock, k["deckB"].keylock), (Some(true), Some(false)));
}

#[test]
fn a_platter_runs_at_target_over_natural_and_holds_pitch_unless_a_hand_is_on_it() {
    near(platter_rate(Some(&tempo(120.0, 60.0)), true), 0.5);
    assert_eq!(platter_rate(Some(&tempo(120.0, 60.0)), false), 0.0);
    assert_eq!(platter_rate(None, true), 1.0);
    let locked = DeckTempo {
        keylock: Some(true),
        ..tempo(120.0, 126.0)
    };
    assert_eq!(platter_key(Some(&locked), false), 1.0);
    assert_eq!(platter_key(Some(&locked), true), 0.0);
    assert_eq!(platter_key(None, false), 0.0);
}

fn load(stems: bool) -> DeckLoad {
    DeckLoad {
        path: "/m/song/original.mp3".into(),
        title: "A".into(),
        artist: None,
        id: "songs/a".into(),
        stems: stems.then_some(true),
        bpm: None,
        key: None,
    }
}

#[test]
fn a_separated_record_is_four_platters_and_an_empty_deck_none() {
    assert_eq!(platters_of("deckA", Some(&load(false))), ["deckA:platterA"]);
    assert_eq!(platters_of("deckA", Some(&load(true))).len(), 4);
    assert!(platters_of("deckA", Some(&load(true)))[0].starts_with("stemDrumsA:platterDrumsA"));
    assert!(platters_of("deckA", None).is_empty());
}

#[test]
fn a_stem_is_beside_its_record_whichever_separator_the_machine_uses() {
    use song_schema::Stem::{Bass, Drums, Vocals};
    assert_eq!(stem_source("/m/song/master.wav", Bass), "/m/song/stems/bass.wav");
    assert_eq!(
        stem_source("C:\\m\\song\\master.wav", Drums),
        "C:\\m\\song\\stems/drums.wav"
    );
    assert_eq!(stem_source("master.wav", Vocals), "stems/vocals.wav");
}

#[test]
fn a_scratch_waits_what_is_left_of_the_pace_and_a_release_never_waits() {
    assert_eq!(scratch_delay(0.0, Some(1.0), None), 16.0);
    assert_eq!(scratch_delay(6.0, Some(1.0), None), 10.0);
    assert_eq!(scratch_delay(100.0, Some(1.0), None), 0.0);
    assert_eq!(scratch_delay(0.0, None, None), 0.0);
}

#[test]
fn a_rebuild_is_worth_it_for_a_moved_target_a_changed_record_or_a_deck_coming_or_going() {
    let a = both();
    assert!(!worth_a_rebuild(&a, &a));
    let mut b = both();
    b["deckA"].target = 126.04;
    assert!(!worth_a_rebuild(&a, &b));
    b["deckA"].target = 126.06;
    assert!(worth_a_rebuild(&a, &b));
    let mut c = both();
    c["deckB"].natural = 91.0;
    assert!(worth_a_rebuild(&a, &c));
    let mut d = both();
    d.shift_remove("deckB");
    assert!(worth_a_rebuild(&a, &d));
}
