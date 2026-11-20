// clips_timelines.rs — a timeline placed as one event, expanded into the events it holds, shifted and cut at its end.
// A reader would get wrong that the expansion happens BEFORE an event is routed to a half: a timeline of clips and notes feeds both.

use crate::clips_support;

use clips_support::{event, grid, world};
use serde_json::{Value, json};
use song::Placement;
use song::clips::{ClipDoc, Materials, Placer, fixed_fields_of};

/// Clips kick (one second long), a sequence of twelve beats, and the timeline `verse` of both.
fn project(verse: Value, more: &[(&str, Value)]) -> Materials {
    let mut v = json!({
        "length": "2:0",
        "lanes": [
            {"lane": "drums", "events": [{"ref": "kick", "start": "1:1"}]},
            {"lane": "bass", "events": [{"ref": "bassline", "start": "1:1"}]},
        ]
    });
    for (k, x) in verse.as_object().expect("object") {
        if x.is_null() {
            v.as_object_mut().expect("object").remove(k);
        } else {
            v[k] = x.clone();
        }
    }
    let mut timelines = vec![("verse", v)];
    timelines.extend(more.iter().cloned());
    world(
        &[("kick", json!({"source": "clips/kick.wav", "to": 1}))],
        &[("bassline", json!({"notes": [{"t": 0, "d": 12, "k": 40}]}))],
        &timelines,
    )
}

fn song(m: &Materials, events: Vec<Value>) -> (Vec<ClipDoc>, Vec<song::Finding>) {
    let evs: Vec<Placement> = events.into_iter().map(event).collect();
    let mut placer = Placer::new(grid(120.0), m);
    let leaves = placer.expand_events(&evs, "nodes/song.k7x");
    let clips = leaves.iter().flat_map(|l| placer.place(l)).collect();
    (clips, placer.finish().0)
}

fn named<'c>(clips: &'c [ClipDoc], name: &str) -> &'c ClipDoc {
    clips.iter().find(|c| c.name == name).expect("a clip")
}

#[test]
fn a_timeline_plays_its_events_through_the_host_lane_shifted_to_where_it_starts() {
    // timelines.test.ts "plays its events through the host lane, shifted to where it starts"
    let (clips, findings) = song(&project(json!({}), &[]), vec![json!({"ref": "verse", "start": "3:1"})]);
    assert!(findings.is_empty());
    let (kick, bass) = (named(&clips, "kick"), named(&clips, "bassline"));
    assert_eq!((kick.start, bass.start), (8.0, 8.0));
    assert_eq!(kick.id, "verse@8>kick@0");
    let nest = kick.nest.as_ref().expect("nest");
    assert_eq!(
        (nest.of.as_str(), nest.ref_.as_str(), nest.start, nest.length),
        ("verse@8", "verse", 8.0, 8.0)
    );
    assert!(nest.event.is_none());
}

#[test]
fn what_runs_past_the_fixed_length_is_cut() {
    // timelines.test.ts "cuts what runs past its fixed length": twelve beats of notes in a two-bar timeline
    let (clips, _) = song(&project(json!({}), &[]), vec![json!({"ref": "verse", "start": "1:1"})]);
    assert_eq!(named(&clips, "bassline").length, 8.0);
}

#[test]
fn a_timeline_with_no_length_is_as_long_as_its_content_rounded_up_to_the_bar() {
    // timelines.test.ts "is as long as its content, rounded up to the bar, when it states no length"
    let m = project(json!({"length": null}), &[]);
    let mut placer = Placer::new(grid(120.0), &m);
    assert_eq!(placer.timeline_length("verse"), 12.0);
    let docs = placer.timeline_docs();
    assert_eq!(docs[0].length, 12.0);
}

#[test]
fn a_length_is_at_least_one_bar_and_zero_zero_is_zero() {
    let none = world(
        &[],
        &[],
        &[
            ("empty", json!({"lanes": [{"lane": "a"}]})),
            ("zero", json!({"length": "0:0", "lanes": [{"lane": "a"}]})),
        ],
    );
    let mut placer = Placer::new(grid(120.0), &none);
    assert_eq!(placer.timeline_length("empty"), 4.0);
    assert_eq!(placer.timeline_length("zero"), 0.0);
    assert_eq!(placer.timeline_length("not-a-timeline"), 0.0);
}

#[test]
fn pictures_extend_a_timeline_and_a_shot_with_no_length_counts_for_nothing() {
    let m = world(
        &[],
        &[],
        &[(
            "tv",
            json!({"lanes": [{"lane": "a"}], "pictures": [{"lane": "p", "events": [{"ref": "x", "start": "2:1", "length": 3}, {"ref": "y", "start": "9:1"}]}]}),
        )],
    );
    // 3 s at 120 bpm is 6 beats from beat 4, and a shot with no length ends where it starts: position 9:1 is beat 32, which is already a whole number of bars
    let mut placer = Placer::new(grid(120.0), &m);
    assert_eq!(placer.timeline_length("tv"), 32.0);
}

#[test]
fn a_timelines_gain_and_mute_reach_every_clip() {
    // timelines.test.ts "carries its gain and its mute onto every clip"; a trim is audio only, summed and clamped once
    let (clips, _) = song(
        &project(json!({}), &[]),
        vec![json!({"ref": "verse", "start": "1:1", "gain": -3, "disabled": true})],
    );
    assert!(clips.iter().all(|c| c.disabled == Some(true)));
    assert_eq!(named(&clips, "kick").gain_db, Some(-3.0));
    assert_eq!(named(&clips, "bassline").gain_db, None);
    let nest = named(&clips, "kick").nest.as_ref().expect("nest");
    let event = nest.event.as_ref().expect("event");
    assert_eq!((event.gain, event.disabled), (Some(-3.0), Some(true)));
}

#[test]
fn a_trim_is_clamped_once_after_the_levels_are_summed() {
    let more = [(
        "outer",
        json!({"lanes": [{"lane": "a", "events": [{"ref": "verse", "start": "1:1", "gain": 5}]}]}),
    )];
    let (clips, _) = song(
        &project(json!({}), &more),
        vec![json!({"ref": "outer", "start": "1:1", "gain": 4})],
    );
    assert_eq!(named(&clips, "kick").gain_db, Some(6.0));
}

#[test]
fn timelines_nest_and_the_outermost_event_names_every_clip() {
    // timelines.test.ts "nests, the outermost event naming every clip"
    let more = [(
        "song-part",
        json!({"lanes": [{"lane": "a", "events": [{"ref": "verse", "start": "2:1"}]}]}),
    )];
    let (clips, findings) = song(
        &project(json!({}), &more),
        vec![json!({"ref": "song-part", "start": "2:1"})],
    );
    assert!(findings.is_empty());
    let kick = named(&clips, "kick");
    assert_eq!(kick.start, 8.0);
    assert_eq!(kick.id, "song-part@4>verse@4>kick@0");
    assert_eq!(kick.nest.as_ref().map(|n| n.of.as_str()), Some("song-part@4"));
}

#[test]
fn a_lane_that_is_disabled_silences_what_it_holds() {
    let m = project(
        json!({"lanes": [
            {"lane": "drums", "disabled": true, "events": [{"ref": "kick", "start": "1:1"}]},
            {"lane": "bass", "events": [{"ref": "bassline", "start": "1:1"}]},
        ]}),
        &[],
    );
    let (clips, _) = song(&m, vec![json!({"ref": "verse", "start": "1:1"})]);
    assert_eq!(named(&clips, "kick").disabled, Some(true));
    assert_eq!(named(&clips, "bassline").disabled, None);
}

#[test]
fn a_clip_wholly_past_the_timelines_end_is_dropped() {
    let m = project(
        json!({"lanes": [{"lane": "a", "events": [{"ref": "kick", "start": "3:1"}, {"ref": "kick", "start": "2:1"}]}]}),
        &[],
    );
    let (clips, _) = song(&m, vec![json!({"ref": "verse", "start": "1:1"})]);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].start, 4.0);
}

#[test]
fn a_clip_with_no_length_takes_the_room_left_and_its_fade_out_is_clamped() {
    let m = world(
        &[("loose", json!({"source": "l.wav"}))],
        &[],
        &[(
            "t",
            json!({"length": "1:0", "lanes": [{"lane": "a", "events": [{"ref": "loose", "start": "1:1", "fadeOut": "9:0"}]}]}),
        )],
    );
    let (clips, _) = song(&m, vec![json!({"ref": "t", "start": "2:1"})]);
    assert_eq!((clips[0].start, clips[0].length, clips[0].fade_out), (4.0, 4.0, 4.0));
}

#[test]
fn a_fixed_field_on_a_timeline_event_is_named_and_still_expands() {
    // decision 169: the refusal is the reference check's
    let p = event(json!({"ref": "verse", "start": "1:1", "loop": 2, "fadeIn": "0:1"}));
    assert_eq!(fixed_fields_of(&p), ["loop", "fadeIn"]);
    assert!(fixed_fields_of(&event(json!({"ref": "verse"}))).is_empty());
    let (clips, _) = song(
        &project(json!({}), &[]),
        vec![json!({"ref": "verse", "start": "1:1", "loop": 2})],
    );
    assert_eq!(clips.len(), 2);
}

#[test]
fn every_timeline_is_compiled_for_its_own_view_its_lanes_addressed_through_it() {
    // timelines.test.ts "compiles every timeline for its own view, lanes addressed through it"
    let m = project(json!({}), &[]);
    let docs = Placer::new(grid(120.0), &m).timeline_docs();
    assert_eq!(docs.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["verse"]);
    assert_eq!(
        docs[0].lanes.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        ["verse>drums", "verse>bass"]
    );
    assert_eq!(docs[0].lanes[0].clips[0].start, 0.0);
    // its events at its own positions, with no nest and no mark of a crossfade
    assert!(docs[0].lanes[0].clips[0].nest.is_none());
}

#[test]
fn a_timeline_docs_color_and_disabled_come_from_its_lane() {
    let m = world(
        &[],
        &[],
        &[(
            "t",
            json!({"lanes": [{"lane": "a", "color": "red", "disabled": true}, {"lane": "b"}]}),
        )],
    );
    let docs = Placer::new(grid(120.0), &m).timeline_docs();
    let lanes = &docs[0].lanes;
    assert_eq!((lanes[0].color.as_str(), lanes[0].disabled), ("red", true));
    assert_eq!((lanes[1].color.as_str(), lanes[1].disabled), ("", false));
}

#[test]
fn the_documents_are_in_family_order() {
    // decision 168: bytewise, as the family is read
    let m = world(
        &[],
        &[],
        &[
            ("a", json!({"lanes": [{"lane": "x"}]})),
            ("b", json!({"lanes": [{"lane": "x"}]})),
            ("c", json!({"lanes": [{"lane": "x"}]})),
        ],
    );
    let ids: Vec<String> = Placer::new(grid(120.0), &m)
        .timeline_docs()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn a_timeline_in_itself_is_cut_silently_and_two_in_a_cycle_have_a_length_either_way() {
    // decision 158, 159: the reference check reports a cycle, here it is only cut
    let m = world(
        &[],
        &[],
        &[
            (
                "a",
                json!({"lanes": [{"lane": "x", "events": [{"ref": "a", "start": "1:1"}, {"ref": "b", "start": "1:1"}]}]}),
            ),
            (
                "b",
                json!({"lanes": [{"lane": "x", "events": [{"ref": "a", "start": "1:1"}]}]}),
            ),
        ],
    );
    let (clips, findings) = song(&m, vec![json!({"ref": "a", "start": "1:1"})]);
    assert!(clips.is_empty() && findings.is_empty());
    // asked in either order, a cycle's lengths are the same: nothing below a cut is kept
    let (mut first, mut second) = (Placer::new(grid(120.0), &m), Placer::new(grid(120.0), &m));
    let a_then_b = (first.timeline_length("a"), first.timeline_length("b"));
    let b_then_a = (second.timeline_length("b"), second.timeline_length("a"));
    assert_eq!(a_then_b, (b_then_a.1, b_then_a.0));
}

#[test]
fn a_nest_past_the_depth_is_cut_with_one_error_on_the_timeline_it_stopped_in() {
    // decision 158: 40 levels of one timeline each
    let items: Vec<(String, Value)> = (0..40)
        .map(|i| {
            let events = if i < 39 {
                json!([{"ref": format!("t{}", i + 1), "start": "1:1"}])
            } else {
                json!([{"ref": "kick", "start": "1:1"}])
            };
            (format!("t{i}"), json!({"lanes": [{"lane": "x", "events": events}]}))
        })
        .collect();
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[("kick", json!({"source": "k.wav"}))], &[], &refs);
    let (clips, findings) = song(&m, vec![json!({"ref": "t0", "start": "1:1"})]);
    assert!(clips.is_empty());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "timelines/t32.k7x");
}

#[test]
fn thirty_one_levels_still_play() {
    let items: Vec<(String, Value)> = (0..31)
        .map(|i| {
            let events = if i < 30 {
                json!([{"ref": format!("t{}", i + 1), "start": "1:1"}])
            } else {
                json!([{"ref": "kick", "start": "1:1"}])
            };
            (
                format!("t{i}"),
                json!({"length": "1:0", "lanes": [{"lane": "x", "events": events}]}),
            )
        })
        .collect();
    let refs: Vec<(&str, Value)> = items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let m = world(&[("kick", json!({"source": "k.wav"}))], &[], &refs);
    let (clips, findings) = song(&m, vec![json!({"ref": "t0", "start": "1:1"})]);
    assert!(findings.is_empty());
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].nest.as_ref().map(|n| n.ref_.as_str()), Some("t0"));
}
