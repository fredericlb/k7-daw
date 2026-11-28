// reads_ports_rows.rs — the lane layout cases the page's `rows` suite held, as the same inputs and rows.
// A reader would get wrong that a row names its lane by index: the shape here prints the ids back.

use serde_json::{Value, json};
use song::reads::*;

fn tracks(v: Value) -> Vec<RTrack> {
    serde_json::from_value(v).expect("tracks")
}

fn ids(v: &[&str]) -> Vec<RTrack> {
    tracks(Value::Array(v.iter().map(|id| json!({ "id": id })).collect()))
}

fn rows_of(
    t: &[RTrack],
    groups: &[&str],
    video: &[&str],
    auto: &[(&str, usize)],
    order: Option<&[&str]>,
) -> Vec<String> {
    let groups: Vec<String> = groups.iter().map(|s| s.to_string()).collect();
    let vid: Vec<String> = video.iter().map(|s| s.to_string()).collect();
    let auto = auto.iter().map(|(k, n)| (k.to_string(), *n)).collect();
    let order: Option<Vec<String>> = order.map(|o| o.iter().map(|s| s.to_string()).collect());
    project_rows(&groups, t, &vid, &auto, order.as_deref())
        .iter()
        .map(|r| match r {
            LayoutRow::Group { .. } => "group".to_owned(),
            LayoutRow::Track { track, .. } => format!("track:{}", t[*track].id),
            LayoutRow::Video { track } => format!("video:{}", video[*track]),
            LayoutRow::Automation { track, lane } => format!("auto:{}:{lane}", t[*track].id),
        })
        .collect()
}

#[test]
fn a_track_whose_group_does_not_exist_is_a_plain_lane() {
    let t = tracks(json!([{ "id": "kick", "group": "gone" }]));
    assert_eq!(rows_of(&t, &[], &[], &[], None), vec!["track:kick"]);
}

#[test]
fn a_groups_members_nest_under_a_header_placed_where_the_first_sits() {
    let t = tracks(json!([{ "id": "bass" }, { "id": "kick", "group": "drums" }, { "id": "snare", "group": "drums" }]));
    assert_eq!(
        rows_of(&t, &["drums"], &[], &[], None),
        vec!["track:bass", "group", "track:kick", "track:snare"]
    );
}

#[test]
fn the_picture_rows_come_last_front_layer_first() {
    let t = ids(&["bass"]);
    assert_eq!(
        rows_of(&t, &[], &["main", "pip"], &[], None),
        vec!["track:bass", "video:pip", "video:main"]
    );
    assert_eq!(rows_of(&t, &[], &[], &[], None), vec!["track:bass"]);
}

#[test]
fn each_picture_row_goes_where_the_track_order_puts_it_among_the_sound_rows() {
    let t = ids(&["drums", "bass"]);
    assert_eq!(
        rows_of(&t, &[], &["main", "pip"], &[], Some(&["pip", "drums", "main", "bass"])),
        vec!["video:pip", "track:drums", "video:main", "track:bass"]
    );
}

#[test]
fn a_group_or_a_lane_is_never_split_from_its_automation_and_an_unlisted_picture_goes_last() {
    let t = tracks(json!([
        { "id": "kick", "group": "kit" }, { "id": "snare#audio", "group": "kit" }, { "id": "bass" }]));
    assert_eq!(
        rows_of(
            &t,
            &["kit"],
            &["main", "loose"],
            &[("bass", 1)],
            Some(&["kick", "main", "snare", "bass"])
        ),
        vec![
            "group",
            "track:kick",
            "track:snare#audio",
            "video:main",
            "track:bass",
            "auto:bass:0",
            "video:loose"
        ]
    );
}

#[test]
fn an_automation_lane_sits_under_its_track_one_row_a_knob() {
    let t = ids(&["gtr"]);
    assert_eq!(
        rows_of(&t, &[], &[], &[("gtr", 2)], None),
        vec!["track:gtr", "auto:gtr:0", "auto:gtr:1"]
    );
}
