// clips_ids_warp.rs — ids written as JavaScript writes them, the grid's order of operations, and a clip file's stretch map.
// A reader would get wrong that an id is compared as a STRING: `1e+21` and `1000000000000000000000` are different ids.

use song::clips::{BUDGET, Budget, Grid, js_number, placement_id, read_clip_warp, timeline_lane_id};
use song::files::{Warp, WindowEdge};
use song_schema::TimeSig;

fn grid(num: u32, den: u32, tempo: f64) -> Grid {
    Grid::new(TimeSig::new(num, den).expect("a signature"), tempo)
}

#[test]
fn a_number_is_written_as_javascript_writes_it() {
    // decision 152
    let cases: [(f64, &str); 18] = [
        (0.0, "0"),
        (-0.0, "0"),
        (8.0, "8"),
        (16.5, "16.5"),
        (-2.0, "-2"),
        (0.1 + 0.2, "0.30000000000000004"),
        (1e21, "1e+21"),
        (1e20, "100000000000000000000"),
        (123456789012345680000.0, "123456789012345680000"),
        (1.5e21, "1.5e+21"),
        (1e-6, "0.000001"),
        (1e-7, "1e-7"),
        (1.5e-7, "1.5e-7"),
        (0.000_001_5, "0.0000015"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (5e-324, "5e-324"),
    ];
    for (v, want) in cases {
        assert_eq!(js_number(v), want, "{v:e}");
    }
    assert_eq!(js_number(1.7976931348623157e308), "1.7976931348623157e+308");
}

#[test]
fn an_id_says_the_material_and_where() {
    // decision 152, after placementId.test.ts "carries the start, so the id says WHERE"
    assert_eq!(placement_id("bass", 8.0), "bass@8");
    assert_eq!(placement_id("bass_2_0", 16.5), "bass_2_0@16.5");
    assert_eq!(placement_id("x", 1e21), "x@1e+21");
    assert_eq!(timeline_lane_id("verse", "drums"), "verse>drums");
}

#[test]
fn the_grid_keeps_the_typescripts_order_of_operations() {
    // decision 153
    let g = grid(4, 4, 120.0);
    assert_eq!(g.bar(), 4.0);
    assert_eq!(grid(6, 8, 120.0).bar(), 3.0);
    assert_eq!(grid(7, 8, 120.0).bar(), 3.5);
    // (beats * 60) / tempo, never beats * (60 / tempo): the two differ in the last bit at 0.1 and 113.
    let g = grid(4, 4, 113.0);
    assert_eq!(g.seconds(0.1), 0.053_097_345_132_743_36);
    assert_ne!(g.seconds(0.1), 0.1 * (60.0 / 113.0));
    let g = grid(4, 4, 120.0);
    assert_eq!(g.seconds(2.0), 1.0);
    assert_eq!(g.beats(0.5), 1.0);
}

#[test]
fn a_start_is_seconds_when_a_number_and_a_position_otherwise() {
    // after secondsStart.test.ts: a NUMBER is seconds at the project's tempo
    let g = grid(4, 4, 120.0);
    assert_eq!(g.position(Some(&WindowEdge::Seconds(4.5))), 9.0);
    assert_eq!(g.position(Some(&WindowEdge::Timecode("3:1:1".into()))), 8.0);
    assert_eq!(g.position(None), 0.0);
    assert_eq!(g.position_text("2:1"), 4.0);
    assert_eq!(g.duration("2:0"), 8.0);
    // decision 156: text that is no timecode is NaN, and a clip then refuses it
    assert!(g.position_text("not a timecode").is_nan());
    assert!(g.duration("").is_nan());
}

fn warp(markers: &[(f64, f64)], disable: Option<bool>) -> Warp {
    let mut v = serde_json::json!({ "algorithm": "elastique", "markers": markers });
    if let Some(d) = disable {
        v["disable"] = d.into();
    }
    serde_json::from_value(v).expect("a warp")
}

#[test]
fn a_map_needs_two_markers() {
    // after clipWarpRead.test.ts "needs TWO markers"
    assert!(read_clip_warp(Some(&warp(&[(0.0, 0.0)], None))).0.is_none());
    let (map, declined) = read_clip_warp(Some(&warp(&[(0.0, 0.0), (4.0, 2.0)], None)));
    assert_eq!(map.expect("a map").markers.len(), 2);
    assert!(!declined);
}

#[test]
fn the_decline_is_answered_apart_because_the_map_survives_it() {
    // after clipWarpRead.test.ts "answers the decline apart from the map"
    let (map, declined) = read_clip_warp(Some(&warp(&[(0.0, 0.0), (4.0, 2.0)], Some(true))));
    assert!(declined);
    assert_eq!(map.expect("kept").markers.len(), 2);
    // a one-marker struct declines with no map at all (decision 154's neighbour: no test of its own in the TypeScript)
    let (none, declined) = read_clip_warp(Some(&warp(&[(0.0, 0.0)], Some(true))));
    assert!(none.is_none() && declined);
}

#[test]
fn a_clip_with_no_warp_block_has_nothing_to_say() {
    // after clipWarpRead.test.ts "has nothing to say about a clip with no warp block"
    assert_eq!(read_clip_warp(None), (None, false));
}

#[test]
fn a_budget_spends_for_good_once_it_is_past() {
    let mut b = Budget::new(10);
    assert!(b.charge(10));
    assert!(!b.capped());
    assert!(!b.charge(1));
    assert!(b.capped() && !b.charge(0));
    assert_eq!(BUDGET, 1_000_000);
}
