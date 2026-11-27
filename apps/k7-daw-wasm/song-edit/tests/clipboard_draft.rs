// clipboard_draft.rs — what a rectangle copies and where a paste lands, and the curve editor's draft, with their TypeScript tests' inputs.
// A reader would get wrong that a copy is RELATIVE to the corner: rows down, beats right, so a paste offsets one way on both axes.

use song_edit::clipboard::*;
use song_edit::curve_draft::*;
use song_schema::{Cut, Polarity};

fn note(key: f64, time: f64, duration: f64, vel: Option<f64>) -> RollNote {
    RollNote {
        key,
        time,
        duration,
        vel,
        repeat: None,
        skip: None,
    }
}

fn notes_of(c: &Option<Clipboard>) -> Vec<CopiedNote> {
    match c {
        Some(Clipboard::Notes { notes, .. }) => notes.clone(),
        other => panic!("not a note clipboard: {other:?}"),
    }
}

fn cn(k: f64, t: f64, d: f64, v: Option<f64>) -> CopiedNote {
    CopiedNote {
        k,
        t,
        d,
        v,
        repeat: None,
        skip: None,
    }
}

fn pn(k: f64, t: f64, d: f64, v: Option<f64>) -> PastedNote {
    PastedNote {
        t,
        d,
        k,
        v,
        repeat: None,
        skip: None,
    }
}

fn five() -> Vec<RollNote> {
    vec![
        note(60.0, 0.0, 1.0, None),
        note(62.0, 1.5, 1.0, None),
        note(48.0, 2.0, 1.0, None),
        note(60.0, 8.0, 1.0, None),
        note(64.0, 3.9, 4.0, None),
    ]
}

#[test]
fn a_copy_takes_what_starts_inside_on_both_axes_and_keeps_a_length_whole() {
    let c = copy_notes(&five(), "riff", 60.0, 67.0, 0.0, 4.0);
    let notes = notes_of(&c);
    assert_eq!(notes.len(), 3);
    assert_eq!(notes.iter().find(|n| n.t == 3.9).map(|n| n.d), Some(4.0));
    // Key 62 in a rectangle topped at 67 is five rows down; beat 1.5 from a left edge of 1 is 0.5.
    let c = copy_notes(&five(), "riff", 60.0, 67.0, 1.0, 4.0);
    assert_eq!(notes_of(&c)[0], cn(5.0, 0.5, 1.0, None));
}

#[test]
fn a_velocity_of_one_is_the_default_and_is_dropped() {
    let c = copy_notes(&[note(60.0, 0.0, 1.0, Some(1.0)), note(61.0, 0.0, 1.0, Some(0.4))], "r", 60.0, 61.0, 0.0, 4.0);
    let n = notes_of(&c);
    assert_eq!(n.iter().find(|n| n.k == 1.0).and_then(|n| n.v), None);
    assert_eq!(n.iter().find(|n| n.k == 0.0).and_then(|n| n.v), Some(0.4));
    assert_eq!(copy_notes(&five(), "riff", 100.0, 110.0, 0.0, 4.0), None);
}

fn riff() -> Clipboard {
    copy_notes(&[note(60.0, 0.0, 1.0, None), note(62.0, 1.5, 2.0, Some(0.4))], "riff", 60.0, 67.0, 0.0, 4.0).unwrap()
}

#[test]
fn a_paste_puts_the_top_left_under_the_caret_and_drops_what_falls_off() {
    let out = paste_notes(&riff(), 67.0, 8.0);
    assert!(out.contains(&pn(60.0, 8.0, 1.0, None)));
    assert!(out.contains(&pn(62.0, 9.5, 2.0, Some(0.4))));
    let mut ks: Vec<f64> = paste_notes(&riff(), 72.0, 0.0).iter().map(|n| n.k).collect();
    ks.sort_by(f64::total_cmp);
    assert_eq!(ks, vec![65.0, 67.0]);
    // A caret on key 6 leaves room for one.
    let out = paste_notes(&riff(), 6.0, 0.0);
    assert_eq!(out, vec![pn(1.0, 1.5, 2.0, Some(0.4))]);
    assert_eq!(dropped_count(out.len(), &riff()), 1);
    assert!(paste_notes(&riff(), 67.0, -4.0).is_empty());
    let Clipboard::Notes { top, .. } = riff() else { panic!() };
    assert_eq!(top, 67.0);
}

fn lane(events: &[(&str, f64, Option<EventAttrs>)]) -> LaneEvents {
    LaneEvents {
        events: events
            .iter()
            .map(|(r, s, e)| LaneEvent {
                reference: (*r).to_owned(),
                start: *s,
                event: e.clone(),
            })
            .collect(),
    }
}

fn events_of(c: &Option<Clipboard>) -> Vec<CopiedEvent> {
    match c {
        Some(Clipboard::Events { events, .. }) => events.clone(),
        other => panic!("not an event clipboard: {other:?}"),
    }
}

fn plain(row: usize, r: &str, start: f64) -> CopiedEvent {
    CopiedEvent {
        row,
        reference: r.into(),
        start,
        attrs: EventAttrs::default(),
    }
}

#[test]
fn an_event_copy_takes_what_starts_inside_and_every_attribute_it_stated() {
    let rows = vec![
        lane(&[("kick", 0.0, None), ("kick", 8.0, None)]),
        lane(&[(
            "snare",
            2.0,
            Some(EventAttrs {
                loops: Some(4.0),
                ..EventAttrs::default()
            }),
        )]),
    ];
    let c = copy_events(&rows, 0.0, 4.0);
    let mut snare = plain(1, "snare", 2.0);
    snare.attrs.loops = Some(4.0);
    assert_eq!(events_of(&c), vec![plain(0, "kick", 0.0), snare]);
    assert!(matches!(c, Some(Clipboard::Events { rows: 2, .. })));
    // One lane and one clip's span copies that clip alone, and bare lane copies nothing.
    let one = copy_events(&rows[..1], 8.0, 12.0);
    assert_eq!(events_of(&one), vec![plain(0, "kick", 0.0)]);
    assert!(matches!(one, Some(Clipboard::Events { rows: 1, beats, .. }) if beats == 4.0));
    assert_eq!(copy_events(&rows[..1], 100.0, 104.0), None);
}

#[test]
fn a_window_keeps_its_spelling_and_a_default_is_left_out() {
    let full = EventAttrs {
        from: Some(WindowEdge::Seconds(1.5)),
        to: Some(WindowEdge::Seconds(6.5455)),
        fade_in: Some("0:2".into()),
        fade_out: Some("0:1".into()),
        loops: Some(3.0),
        disabled: Some(true),
        warp: Some(false),
    };
    let c = copy_events(&[lane(&[("gtr", 0.0, Some(full.clone()))])], 0.0, 4.0);
    assert_eq!(events_of(&c)[0].attrs, full);
    let tc = EventAttrs {
        from: Some(WindowEdge::Timecode("2:1".into())),
        to: Some(WindowEdge::Timecode("4:1".into())),
        ..EventAttrs::default()
    };
    let c = copy_events(&[lane(&[("s", 0.0, Some(tc.clone()))])], 0.0, 4.0);
    assert_eq!(events_of(&c)[0].attrs, tc);
    let defaults = EventAttrs {
        loops: Some(1.0),
        warp: Some(true),
        ..EventAttrs::default()
    };
    let c = copy_events(&[lane(&[("a", 0.0, Some(defaults))])], 0.0, 4.0);
    assert_eq!(events_of(&c), vec![plain(0, "a", 0.0)]);
}

#[test]
fn an_event_paste_lands_from_the_caret_down_and_drops_the_rows_with_no_lane() {
    let clip = copy_events(&[lane(&[("kick", 0.0, None)]), lane(&[("snare", 2.0, None)])], 0.0, 4.0).unwrap();
    let lanes = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let out = paste_events(&clip, &lanes(&["gtr", "bass"]), 16.0);
    let got: Vec<_> = out.iter().map(|e| (e.track.as_str(), e.reference.as_str(), e.start)).collect();
    assert_eq!(got, vec![("gtr", "kick", 16.0), ("bass", "snare", 18.0)]);
    let out = paste_events(&clip, &lanes(&["gtr"]), 0.0);
    assert_eq!(out.len(), 1);
    assert_eq!(dropped_count(out.len(), &clip), 1);
    assert!(paste_events(&riff(), &lanes(&["gtr"]), 0.0).is_empty());
    assert!(paste_notes(&clip, 60.0, 0.0).is_empty());
}

#[test]
fn a_paste_moves_the_start_and_leaves_the_materials_coordinates_alone() {
    let a = EventAttrs {
        from: Some(WindowEdge::Seconds(1.5)),
        to: Some(WindowEdge::Seconds(6.5455)),
        fade_in: Some("0:2".into()),
        loops: Some(3.0),
        ..EventAttrs::default()
    };
    let c = copy_events(&[lane(&[("gtr", 4.0, Some(a.clone()))])], 4.0, 8.0).unwrap();
    let out = paste_events(&c, &["bass".to_owned()], 64.0);
    assert_eq!((out[0].track.as_str(), out[0].start), ("bass", 64.0));
    assert_eq!(out[0].attrs, a);
}

#[test]
fn a_note_is_in_a_rectangle_when_it_starts_inside_with_a_hair_of_tolerance() {
    let notes = [(60.0, 0.0), (60.0, 4.0), (60.0, 3.9999999999), (59.0, 1.0)];
    assert_eq!(notes_in_region(&notes, 60.0, 61.0, 0.0, 4.0), vec![0]);
}

fn custom() -> CurveDraft {
    CurveDraft {
        shape: DraftShape::Custom,
        ..draft_of(None)
    }
}

fn keys(m: &serde_json::Map<String, serde_json::Value>) -> Vec<&str> {
    m.keys().map(String::as_str).collect()
}

#[test]
fn the_fields_written_are_the_shapes_own() {
    let saw = CurveDraft {
        shape: DraftShape::Saw,
        amplitude: 0.5,
        phase: 0.25,
        ..draft_of(None)
    };
    let f = fields_of(&saw).unwrap();
    assert_eq!(keys(&f), vec!["type", "amplitude", "shape", "frequency", "phase"]);
    assert_eq!(f["shape"], "saw");
    let adsr = fields_of(&CurveDraft {
        shape: DraftShape::Adsr,
        ..draft_of(None)
    })
    .unwrap();
    let mut k = keys(&adsr);
    k.sort_unstable();
    assert_eq!(k, vec!["amplitude", "attack", "decay", "level", "release", "sustain", "type"]);
    assert_eq!(fields_of(&custom()).unwrap()["points"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_curve_opens_on_its_own_fields_and_keeps_its_polarity_on_the_way_out() {
    let file: SpecFile = serde_json::from_value(serde_json::json!({
        "type": "lfo", "polarity": "bipolar", "amplitude": 0.3, "shape": "triangle", "frequency": 2, "phase": 0.5
    }))
    .unwrap();
    let d = draft_of(Some(&file));
    assert_eq!(d.shape, DraftShape::Triangle);
    assert_eq!(fields_of(&d).unwrap()["polarity"], "bipolar");
    assert_eq!(*keys(&fields_of(&d).unwrap()).last().unwrap(), "polarity");
}

#[test]
fn an_onoff_curve_is_its_points_off_or_on_and_its_conditions() {
    let d = CurveDraft {
        shape: DraftShape::Onoff,
        amplitude: 0.3,
        polarity: Polarity::Bipolar,
        points: vec![
            DraftPoint {
                time: "1:1:1".into(),
                value: 0.2,
            },
            DraftPoint {
                time: "2:1:1".into(),
                value: 0.9,
            },
        ],
        ..draft_of(None)
    };
    let f = serde_json::Value::Object(fields_of(&d).unwrap());
    assert_eq!(
        f,
        serde_json::json!({ "type": "onoff", "points": [{ "time": "1:1:1", "value": 0.0 }, { "time": "2:1:1", "value": 1.0 }] })
    );
    assert_eq!(shapes_for(Some(ParamKind::Boolean)), vec![DraftShape::Onoff]);
    assert!(!shapes_for(Some(ParamKind::Number)).contains(&DraftShape::Onoff));
    assert!(shapes_for(Some(ParamKind::Number)).contains(&DraftShape::Custom));
    assert!(shapes_for(None).contains(&DraftShape::Onoff));
}

#[test]
fn a_drawn_point_snaps_to_a_sixteenth_and_one_point_stays_per_moment() {
    let p = point_at(0.252, 0.5, 16.0, 4.0, 4.0, false).unwrap();
    assert_eq!((p.time.as_str(), p.value), ("2:1", 0.5));
    assert_eq!(point_at(0.0, 0.7, 16.0, 4.0, 4.0, true).unwrap().value, 1.0);
    assert_eq!(point_at(0.0, 0.3, 16.0, 4.0, 4.0, true).unwrap().value, 0.0);
    let pts = vec![
        DraftPoint {
            time: "1:1:1".into(),
            value: 0.0,
        },
        DraftPoint {
            time: "3:1:1".into(),
            value: 1.0,
        },
    ];
    let at = |t: &str, v: f64| DraftPoint {
        time: t.into(),
        value: v,
    };
    let times = |v: Vec<DraftPoint>| v.into_iter().map(|p| p.time).collect::<Vec<_>>();
    assert_eq!(times(with_point(&pts, &at("2:1:1", 0.5), 4.0, 4.0, None).unwrap()), vec!["1:1:1", "2:1:1", "3:1:1"]);
    // `1:1` is the moment `1:1:1` is, however it is spelt.
    assert_eq!(with_point(&pts, &at("1:1", 0.9), 4.0, 4.0, None).unwrap().len(), 2);
    assert_eq!(times(with_point(&pts, &at("4:1:1", 1.0), 4.0, 4.0, Some("3:1:1")).unwrap()), vec!["1:1:1", "4:1:1"]);
    assert!(with_point(&pts, &at("1:", 0.0), 4.0, 4.0, None).is_err());
}

#[test]
fn the_plot_spans_four_bars_or_past_the_last_point() {
    assert_eq!(span_bars(&custom(), 4.0, 4.0).unwrap(), 4.0);
    let d = CurveDraft {
        points: vec![DraftPoint {
            time: "6:1:1".into(),
            value: 1.0,
        }],
        ..custom()
    };
    assert_eq!(span_bars(&d, 4.0, 4.0).unwrap(), 7.0);
    assert_eq!(span_bars(&draft_of(None), 4.0, 4.0).unwrap(), 4.0);
}

fn cond() -> DraftCondition {
    DraftCondition {
        start_at: "3:1:1".into(),
        end_at: "7:1:1".into(),
        ..new_condition("Note On")
    }
}

fn written(cs: Vec<DraftCondition>) -> Option<serde_json::Value> {
    let d = CurveDraft {
        conditions: cs,
        ..draft_of(None)
    };
    fields_of(&d).unwrap().get("conditions").cloned()
}

#[test]
fn each_condition_kind_writes_only_its_own_moments_in_the_order_given() {
    assert_eq!(written(vec![]), None);
    let when = |w: DraftWhen| DraftCondition { when: w, ..cond() };
    assert_eq!(written(vec![when(DraftWhen::Start)]), Some(serde_json::json!([{ "type": "start", "at": "3:1:1" }])));
    let between = DraftCondition {
        retrigger: true,
        ..when(DraftWhen::Between)
    };
    assert_eq!(
        written(vec![between]),
        Some(serde_json::json!([{ "type": "between", "start": "3:1:1", "end": "7:1:1", "retrigger": true }]))
    );
    assert_eq!(written(vec![when(DraftWhen::Event)]), Some(serde_json::json!([{ "type": "event", "event": "Note On" }])));
    assert_eq!(
        written(vec![when(DraftWhen::End), when(DraftWhen::Start)]),
        Some(serde_json::json!([{ "type": "end", "at": "7:1:1" }, { "type": "start", "at": "3:1:1" }]))
    );
}

#[test]
fn a_curve_opens_on_its_own_conditions() {
    let file: SpecFile = serde_json::from_value(serde_json::json!({
        "type": "lfo", "polarity": "unipolar", "amplitude": 1, "shape": "sine", "frequency": 1, "phase": 0,
        "conditions": [{ "type": "end", "at": "9:1:1", "retrigger": true }, { "type": "event", "event": "Note On" }]
    }))
    .unwrap();
    let d = draft_of(Some(&file));
    let got: Vec<_> = d.conditions.iter().map(|c| (c.when, c.retrigger)).collect();
    assert_eq!(got, vec![(DraftWhen::End, true), (DraftWhen::Event, false)]);
    assert_eq!(d.conditions[0].end_at, "9:1:1");
}

#[test]
fn what_stops_a_save_is_named() {
    let when = |w: DraftWhen| DraftCondition { when: w, ..cond() };
    let soon = DraftCondition {
        start_at: "soon".into(),
        ..when(DraftWhen::Start)
    };
    assert!(condition_problem(&soon).unwrap().contains("not a timecode"));
    let blank = DraftCondition {
        event: " ".into(),
        ..when(DraftWhen::Event)
    };
    assert!(condition_problem(&blank).unwrap().contains("event"));
    assert_eq!(condition_problem(&when(DraftWhen::Between)), None);
    let later = DraftCondition {
        end_at: "later".into(),
        ..when(DraftWhen::End)
    };
    let d = CurveDraft {
        conditions: vec![cond(), later],
        ..draft_of(None)
    };
    assert!(conditions_problem(&d).unwrap().contains("later"));
}

#[test]
fn an_oscillators_duration_is_written_when_finite_and_checked_when_typed() {
    let sine = CurveDraft {
        shape: DraftShape::Sine,
        frequency: 2.0,
        ..draft_of(None)
    };
    assert!(!fields_of(&sine).unwrap().contains_key("duration"));
    let two = CurveDraft {
        duration: "2:0".into(),
        ..sine.clone()
    };
    assert_eq!(fields_of(&two).unwrap()["duration"], "2:0");
    let soon = CurveDraft {
        duration: "soon".into(),
        ..sine.clone()
    };
    assert!(duration_problem(&soon).unwrap().contains("length"));
    assert_eq!(duration_problem(&sine), None);
    assert_eq!(draft_of(None).cut, Cut::None);
}
