// reads_ports_takes.rs — the takes cases the page's `takes` suite held that `reads.rs` lacked, as the same inputs and lines.
// A reader would get wrong the spans: the comp here is the page's, 6..8, where `reads.rs` heard 6..7.

use serde_json::{Value, json};
use song::reads::*;

fn clips(v: Value) -> Vec<RClip> {
    serde_json::from_value(v).expect("clips")
}

fn comped() -> Vec<RClip> {
    clips(json!([
        { "id": "riff@4", "type": "midi", "name": "riff", "start": 4, "length": 2, "comp": { "of": "riff@4", "take": "riff" },
          "event": { "takes": [{ "ref": "t2", "start": "2:3" }], "comp": [{ "take": "t2", "from": "2:3", "to": "3:1" }], "crossfades": [{ "at": "2:3", "length": "0:0:1" }] } },
        { "id": "t2@6", "type": "midi", "name": "t2", "start": 6, "length": 2, "comp": { "of": "riff@4", "take": "t2" } },
        { "id": "other@16", "type": "midi", "name": "other", "start": 16, "length": 4 },
    ]))
}

#[test]
fn an_event_reads_its_span_and_the_comp_in_beats() {
    let ev = takes_event_of("keys", "riff@4", &comped(), 4.0, 4.0)
        .expect("reads")
        .expect("an event");
    assert_eq!(
        (ev.ref_.as_str(), ev.start, ev.at.as_str(), ev.from, ev.to),
        ("riff", 4.0, "2:1", 4.0, 8.0)
    );
    let comp: Vec<(&str, f64, f64)> = ev.comp.iter().map(|c| (c.take.as_str(), c.from, c.to)).collect();
    assert_eq!(comp, vec![("t2", 6.0, 8.0)]);
    assert_eq!(ev.crossfades.len(), 1);
    assert_eq!((ev.crossfades[0].at, ev.crossfades[0].length), (6.0, 0.5));
}

#[test]
fn a_placement_repeated_at_one_beat_is_found_by_its_number_two_id() {
    let own = &comped()[2];
    let mut repeat = own.clone();
    repeat.id = "other@16#2".to_owned();
    let twice = vec![own.clone(), repeat];
    let ev = takes_event_of("keys", "other@16#2", &twice, 4.0, 4.0)
        .expect("reads")
        .expect("an event");
    assert_eq!((ev.ref_.as_str(), ev.start), ("other", 16.0));
}

#[test]
fn the_comp_is_heard_over_a_takes_spans_and_the_own_row_where_none_is() {
    let comp = vec![("t2".to_owned(), 6.0, 8.0)];
    assert_eq!(heard_spans(&comp, false, "t2", 6.0, 10.0), vec![(6.0, 8.0)]);
    assert_eq!(
        heard_spans(&comp, true, "riff", 4.0, 12.0),
        vec![(4.0, 6.0), (8.0, 12.0)]
    );
}

#[test]
fn a_pick_and_an_arm_are_spelled_in_timecodes() {
    assert_eq!(
        comp_line("keys", "2:1", "t2", 6.0, 8.0, 4.0, 4.0).as_deref(),
        Ok("event comp keys 2:1 t2 2:3 3:1")
    );
    assert_eq!(
        arm_line("keys", "2:1", 6.0, Some(8.0), 4.0, 4.0).as_deref(),
        Ok("event take arm keys 2:1 2:3 3:1")
    );
}

#[test]
fn a_crossfade_line_names_a_seam_and_its_length() {
    assert_eq!(comp_seams(4.0, 8.0, &[(6.0, 8.0)]), vec![6.0]);
    assert_eq!(
        comp_fade_line("keys", "2:1", 6.0, 0.5, 4.0, 4.0).as_deref(),
        Ok("event comp fade keys 2:1 2:3 0:0:1")
    );
}

#[test]
fn a_shrunk_piece_gives_back_what_it_lets_go_to_the_piece_it_touches() {
    let all = comped();
    let pieces = &all[..2];
    let take = piece_edge_lines("keys", "2:1", "riff", pieces, "t2@6", 7.0, 8.0, 4.0, 4.0).expect("lines");
    assert_eq!(take, vec!["event comp keys 2:1 riff 2:3 2:4"]);
    let own = piece_edge_lines("keys", "2:1", "riff", pieces, "riff@4", 4.0, 5.0, 4.0, 4.0).expect("lines");
    assert_eq!(own, vec!["event comp keys 2:1 t2 2:2 2:3"]);
}
