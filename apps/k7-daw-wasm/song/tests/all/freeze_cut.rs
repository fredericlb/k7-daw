// freeze_cut.rs — the document cut down to one plan (decision 342): which tracks stay, what is neutral, and what a cut costs.
// A reader would get wrong that the cut is charged before it is copied: a spent budget answers None and nothing was cloned.

use crate::freeze_support;

use freeze_support::{doc, fast, send, track};
use indexmap::IndexSet;
use song::clips::{Budget, ClipDoc, ClipKind, NoteDoc};
use song::doc::GroupDoc;
use song::freeze::{BUDGET, FreezePlan, freeze_cut};

fn plan(lanes: &[&str], modules: &[&str]) -> FreezePlan {
    FreezePlan {
        at: "amp".into(),
        render: String::new(),
        lanes: lanes.iter().map(|s| (*s).to_owned()).collect::<IndexSet<_>>(),
        modules: modules.iter().map(|s| (*s).to_owned()).collect::<IndexSet<_>>(),
    }
}

fn clip(notes: usize) -> ClipDoc {
    ClipDoc {
        id: "c".into(),
        kind: ClipKind::Midi,
        name: "c".into(),
        source: String::new(),
        start: 0.0,
        length: 4.0,
        offset: 0.0,
        source_from: None,
        source_to: None,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: (0..notes)
            .map(|i| NoteDoc {
                time: i as f64,
                duration: 1.0,
                key: 60,
                vel: 1.0,
                channel: 0,
                rel: 0.0,
                repeat: None,
                skip: None,
            })
            .collect(),
        warp: None,
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

fn scene() -> song::doc::ProjectDoc {
    let mut gtr = track("gtr", &["amp", "cab"]);
    gtr.solo = true;
    gtr.sends = vec![send("bus"), send("elsewhere")];
    gtr.clips = vec![clip(3)];
    let mut d = doc(vec![gtr, track("bus", &["verb"]), track("other", &["delay"])]);
    d.master_volume = -6.0;
    d.master_plugins = vec![song::doc::PluginDoc::new("master:limiter")];
    d.groups = vec![GroupDoc {
        id: "g".into(),
        name: "g".into(),
        color: String::new(),
        mute: false,
        solo: true,
    }];
    d
}

#[test]
fn a_cut_keeps_a_lane_of_the_plan_and_a_track_holding_a_module_of_it() {
    // decision 342
    let cut = freeze_cut(&scene(), &plan(&["gtr"], &["amp", "verb"]), &mut Budget::new(BUDGET)).expect("budget");
    let ids: Vec<&str> = cut.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["gtr", "bus"]);
}

#[test]
fn a_cut_keeps_only_the_plugins_of_the_plan_and_the_sends_between_kept_tracks() {
    // decision 342: a send leaving the plan has nowhere to land
    let cut = freeze_cut(&scene(), &plan(&["gtr"], &["amp", "verb"]), &mut Budget::new(BUDGET)).expect("budget");
    let gtr = &cut.tracks[0];
    assert_eq!(
        gtr.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["gtr:amp"]
    );
    assert_eq!(gtr.sends.iter().map(|s| s.to.as_str()).collect::<Vec<_>>(), ["bus"]);
    assert_eq!(gtr.clips.len(), 1);
}

#[test]
fn a_cut_is_neutral_at_the_master_and_silences_every_solo() {
    // decision 342
    let cut = freeze_cut(&scene(), &plan(&["gtr"], &["amp"]), &mut Budget::new(BUDGET)).expect("budget");
    assert_eq!(cut.master_volume, 0.0);
    assert!(cut.master_plugins.is_empty());
    assert!(cut.tracks.iter().all(|t| !t.solo));
}

#[test]
fn a_cut_keeps_the_rest_of_the_document() {
    // decision 342: `...doc` in the TypeScript
    let cut = freeze_cut(&scene(), &plan(&["gtr"], &["amp"]), &mut Budget::new(BUDGET)).expect("budget");
    assert_eq!((cut.tempo, cut.sample_rate), (120.0, 48000));
    assert_eq!(cut.groups.len(), 1);
}

#[test]
fn a_plan_of_nothing_cuts_every_track_away() {
    // freeze_cut hostile row, "a plan of nothing"; decision 342
    let cut = freeze_cut(&scene(), &plan(&[], &[]), &mut Budget::new(BUDGET)).expect("budget");
    assert!(cut.tracks.is_empty());
}

#[test]
fn a_cut_past_the_budget_is_none_and_costs_what_it_charged() {
    // freeze_cut hostile row: 301 tracks of 10,000 notes each are past the real budget, which is 3,000,000
    let big = clip(10_000);
    let tracks: Vec<_> = (0..301)
        .map(|i| {
            let mut t = track(&format!("t{i}"), &[]);
            t.clips = vec![big.clone()];
            t
        })
        .collect();
    let d = doc(tracks);
    let lanes: Vec<String> = (0..301).map(|i| format!("t{i}")).collect();
    let lane_refs: Vec<&str> = lanes.iter().map(String::as_str).collect();
    let mut budget = Budget::new(BUDGET);
    let cut = fast("a capped cut", || freeze_cut(&d, &plan(&lane_refs, &[]), &mut budget));
    assert!(cut.is_none() && budget.capped());
    // Charged before the copy: no more than one track's weight past the limit.
    assert!(budget.spent() < BUDGET + 10_100, "{}", budget.spent());
}

/// How many cuts of `d` one budget pays for before it answers None, out of twenty.
fn cuts_paid(d: &song::doc::ProjectDoc) -> usize {
    let mut budget = Budget::new(BUDGET);
    (0..20)
        .take_while(|_| freeze_cut(d, &plan(&["gtr"], &[]), &mut budget).is_some())
        .count()
}

fn heavy(text: &str) -> String {
    text.repeat(32 * 1024 * 1024)
}

#[test]
fn the_text_of_the_rest_of_the_document_is_charged_in_bytes() {
    // decision 342, 348: a 32 MiB string is 2,097,152 of the budget each time it is copied, wherever it sits
    type Make = Box<dyn Fn() -> song::doc::ProjectDoc>;
    let cases: Vec<(&str, Make)> = vec![
        (
            "a marker",
            Box::new(|| {
                let mut d = scene();
                d.markers.push(song::files::Marker {
                    tc: "1:1".into(),
                    name: heavy("m"),
                    beat: 0.0,
                });
                d
            }),
        ),
        (
            "the name",
            Box::new(|| {
                let mut d = scene();
                d.name = heavy("n");
                d
            }),
        ),
        (
            "the record dir",
            Box::new(|| {
                let mut d = scene();
                d.record_dir = heavy("r");
                d
            }),
        ),
        (
            "the uuid",
            Box::new(|| {
                let mut d = scene();
                d.uuid = Some(heavy("u"));
                d
            }),
        ),
        (
            "a group",
            Box::new(|| {
                let mut d = scene();
                d.groups[0].name = heavy("g");
                d
            }),
        ),
        (
            "a timeline clip",
            Box::new(|| {
                let mut d = scene();
                let mut c = clip(1);
                c.source = heavy("s");
                d.timelines = Some(vec![song::clips::TimelineDoc {
                    id: "tl".into(),
                    length: 4.0,
                    lanes: vec![song::clips::TimelineLaneDoc {
                        id: "l".into(),
                        name: "l".into(),
                        color: String::new(),
                        disabled: false,
                        clips: vec![c],
                    }],
                }]);
                d
            }),
        ),
    ];
    for (what, make) in &cases {
        assert_eq!(cuts_paid(&make()), 1, "{what}");
    }
    assert_eq!(cuts_paid(&scene()), 20);
}

#[test]
fn the_text_of_a_kept_clip_is_charged_too() {
    // decision 342: ClipDoc::weight counts no string, the cut adds them
    let mut d = scene();
    d.tracks[0].clips[0].source = heavy("s");
    assert_eq!(cuts_paid(&d), 1);
}

#[test]
fn a_cut_charges_the_whole_weight_of_what_it_keeps() {
    // decision 342, 348: a kept track costs its clips and notes too
    let mut t = track("gtr", &[]);
    t.clips = vec![clip(500)];
    let d = doc(vec![t]);
    let mut budget = Budget::new(BUDGET);
    freeze_cut(&d, &plan(&["gtr"], &[]), &mut budget).expect("budget");
    assert!(budget.spent() >= 500, "{}", budget.spent());
}
