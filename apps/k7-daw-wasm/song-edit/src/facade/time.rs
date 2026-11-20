// time.rs — the time side's arms: placements' gain, loops and cuts, take comps, the project's time splice, the onsets.
// A reader would get wrong `halveLoopsOf`'s snapshot: the host's `Fs` sees each write, so the arm passes a callback that does nothing.

use super::args::{at, done, floats, n, s, undefined};
use super::{Ctx, Failure, Out};
use crate::clip_resize::{Bounds, MaterialKind, ResizeBefore, ResizeTo, resize_to_window};
use crate::crossfade::{CrossfadeSide, plan_crossfade};
use crate::event_gain::{event_gain_value, set_track_placement_gain};
use crate::event_loop::{At, set_track_placement_loop, set_track_placement_loop_points};
use crate::insert_time::splice_project_time;
use crate::material_cut::{consolidate_sequence_material, split_audio_placement, split_sequence_material};
use crate::material_notes::Note;
use crate::material_window::{PlacedExtent, material_window};
use crate::onsets::{OnsetEdits, add_onset, move_onset, remove_onset, resolve_onsets};
use crate::sequence_loops::halve_loops_of;
use crate::song_tempo::song_tempo;
use crate::take_edit::{
    EventAt, add_event_take, remove_event_take, set_event_comp, set_event_crossfade, write_take_sequence,
};
use crate::take_file::Span;
use crate::time_splice::Mode;
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[
    (
        "song-edit/materialCut",
        &[
            "splitSequenceMaterial",
            "consolidateSequenceMaterial",
            "splitAudioPlacement",
        ],
    ),
    ("song-edit/materialWindow", &["materialWindow"]),
    ("song-edit/clipResize", &["resizeToWindow"]),
    ("song-edit/crossfade", &["planCrossfade"]),
    ("song-edit/eventGain", &["eventGainValue", "setTrackPlacementGain"]),
    (
        "song-edit/eventLoop",
        &["setTrackPlacementLoop", "setTrackPlacementLoopPoints"],
    ),
    (
        "song-edit/takeEdit",
        &[
            "setEventComp",
            "setEventCrossfade",
            "removeEventTake",
            "addEventTake",
            "writeTakeSequence",
        ],
    ),
    ("song-edit/insertTime", &["spliceProjectTime"]),
    ("song-edit/sequenceLoops", &["halveLoopsOf"]),
    ("song-edit/songTempo", &["songTempo"]),
    (
        "song-edit/onsets",
        &["resolveOnsets", "removeOnset", "addOnset", "moveOnset"],
    ),
];

fn present(j: &Json) -> Option<&Json> {
    (!j.is_null() && j.get("$").is_none()).then_some(j)
}

fn field(j: &Json, k: &str) -> Option<f64> {
    j.get(k).and_then(Json::as_f64)
}

fn edits_of(j: Option<&Json>) -> Option<OnsetEdits> {
    let j = j.and_then(present)?;
    let list = |k: &str| {
        j.get(k)
            .and_then(Json::as_array)
            .map(|a| a.iter().filter_map(Json::as_f64).collect())
    };
    Some(OnsetEdits {
        added: list("added"),
        removed: list("removed"),
    })
}

fn edits_json(e: &OnsetEdits) -> Json {
    let mut o = serde_json::Map::new();
    if let Some(a) = &e.added {
        o.insert("added".into(), json!(a));
    }
    if let Some(r) = &e.removed {
        o.insert("removed".into(), json!(r));
    }
    Json::Object(o)
}

fn side(j: &Json) -> CrossfadeSide {
    CrossfadeSide {
        start: field(j, "start").unwrap_or(f64::NAN),
        length: field(j, "length").unwrap_or(f64::NAN),
        from: field(j, "from").unwrap_or(f64::NAN),
        to: field(j, "to"),
        mat_from: field(j, "matFrom").unwrap_or(f64::NAN),
        mat_to: field(j, "matTo"),
    }
}

fn kind_of(j: &Json) -> MaterialKind {
    if j["kind"] == "sequence" {
        MaterialKind::Sequence
    } else {
        MaterialKind::Audio
    }
}

fn notes_of(j: &Json) -> Vec<Note> {
    j.as_array()
        .map(|a| {
            a.iter()
                .map(|x| Note {
                    t: field(x, "t").unwrap_or(f64::NAN),
                    d: field(x, "d").unwrap_or(f64::NAN),
                    k: field(x, "k").unwrap_or(f64::NAN),
                    v: x.get("v").and_then(Json::as_f64).map(k7x::edit::Value::Num),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn written(w: &crate::material_notes::WrittenMaterial) -> Json {
    json!({ "ref": w.ref_, "file": w.file, "notes": w.notes })
}

fn kept_json(k: &crate::track_file::KeptLoop, lane: &str) -> Json {
    let start = match &k.start {
        k7x::edit::Value::Num(n) => json!(n),
        k7x::edit::Value::Str(s) => json!(s),
        _ => Json::Null,
    };
    let why = match k.why {
        crate::track_file::KeptWhy::Odd => "odd",
        crate::track_file::KeptWhy::Windowed => "windowed",
    };
    json!({ "lane": lane, "loop": k.loop_, "start": start, "why": why })
}

fn event_at<'a>(dir: &'a str, track: &'a str, beats: f64, num: f64, den: f64) -> EventAt<'a> {
    EventAt {
        dir,
        track,
        event_beats: beats,
        num,
        den,
    }
}

pub fn run(cx: &mut Ctx, _module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let dir = s(a, 0);
    let meter = |i: usize| (n(a, i), n(a, i + 1));
    let track = s(a, 1);
    Some(match f {
        "songTempo" => done(song_tempo(fs, &dir), |t| json!(t)),
        "eventGainValue" => Ok(event_gain_value(n(a, 0)).map_or_else(undefined, |v| json!(v))),
        "setTrackPlacementGain" => done(
            set_track_placement_gain(fs, &dir, &track, n(a, 2), meter(3), n(a, 5)),
            |b| json!(b),
        ),
        "setTrackPlacementLoop" => {
            let at_ = At {
                dir: &dir,
                id: &track,
                from_beats: n(a, 2),
                num: n(a, 3),
                den: n(a, 4),
                tempo: n(a, 5),
            };
            let secs = a.get(7).and_then(present).and_then(Json::as_f64);
            done(
                set_track_placement_loop(fs, &at_, at(a, 6).as_bool().unwrap_or(false), secs),
                |r| r.map_or(Json::Null, |r| json!({ "loops": r.loops, "changed": r.changed })),
            )
        }
        "setTrackPlacementLoopPoints" => {
            let at_ = At {
                dir: &dir,
                id: &track,
                from_beats: n(a, 2),
                num: n(a, 3),
                den: n(a, 4),
                tempo: n(a, 5),
            };
            done(set_track_placement_loop_points(fs, &at_, n(a, 6), n(a, 7)), |r| {
                r.map_or(
                    Json::Null,
                    |r| json!({ "kind": r.kind.name(), "from": r.from, "to": r.to, "loops": r.loops }),
                )
            })
        }
        "splitSequenceMaterial" => done(
            split_sequence_material(fs, &dir, &track, n(a, 2), n(a, 3), meter(4)),
            |r| {
                r.map_or(
                    Json::Null,
                    |r| json!({ "head": written(&r.head), "tail": written(&r.tail), "from": r.from }),
                )
            },
        ),
        "consolidateSequenceMaterial" => done(
            consolidate_sequence_material(fs, &dir, &track, n(a, 2), n(a, 3), meter(4)),
            |r| {
                r.map_or(
                    Json::Null,
                    |r| json!({ "written": written(&r.written), "replaced": r.replaced, "from": r.from }),
                )
            },
        ),
        "splitAudioPlacement" => done(
            split_audio_placement(fs, &dir, &track, n(a, 2), n(a, 3), meter(4), n(a, 6)),
            |r| r.map_or(Json::Null, |cut| json!({ "cut": cut })),
        ),
        "spliceProjectTime" => {
            let Some(mode) = Mode::parse(&s(a, 3)) else {
                return Some(Err(Failure::Unsupported("an unknown splice mode")));
            };
            done(
                splice_project_time(fs, &dir, n(a, 1), n(a, 2), mode),
                |r| json!({ "files": r.files, "moved": r.moved, "removed": r.removed }),
            )
        }
        "halveLoopsOf" => {
            let mut keep = |_: &str| {};
            done(halve_loops_of(fs, &dir, &track, Some(&mut keep)), |r| {
                let kept: Vec<Json> = r.kept.iter().map(|(k, lane)| kept_json(k, lane)).collect();
                json!({ "files": r.files, "halved": r.halved, "kept": kept })
            })
        }
        "setEventComp" => {
            let e = event_at(&dir, &track, n(a, 2), n(a, 4), n(a, 5));
            let sp = at(a, 3);
            let (t, fr, to) = (
                sp["take"].as_str().unwrap_or("").to_owned(),
                sp["from"].as_str().unwrap_or("").to_owned(),
                sp["to"].as_str().unwrap_or("").to_owned(),
            );
            done(
                set_event_comp(
                    fs,
                    &e,
                    &Span {
                        take: &t,
                        from: &fr,
                        to: &to,
                    },
                ),
                |p| json!(p),
            )
        }
        "setEventCrossfade" => {
            let e = event_at(&dir, &track, n(a, 2), n(a, 5), n(a, 6));
            let len = at(a, 4).as_str().map(str::to_owned);
            done(set_event_crossfade(fs, &e, &s(a, 3), len.as_deref()), |p| json!(p))
        }
        "removeEventTake" => {
            let e = event_at(&dir, &track, n(a, 2), n(a, 4), n(a, 5));
            done(remove_event_take(fs, &e, &s(a, 3)), |p| json!(p))
        }
        "addEventTake" => {
            let e = event_at(&dir, &track, n(a, 2), n(a, 5), n(a, 6));
            let (r, st) = (
                at(a, 3)["ref"].as_str().unwrap_or("").to_owned(),
                at(a, 3)["start"].as_str().unwrap_or("").to_owned(),
            );
            let span = a.get(4).filter(|j| j.is_object()).map(|j| {
                (
                    j["from"].as_str().unwrap_or("").to_owned(),
                    j["to"].as_str().unwrap_or("").to_owned(),
                )
            });
            let span_ref = span.as_ref().map(|(x, y)| (x.as_str(), y.as_str()));
            done(add_event_take(fs, &e, (&r, &st), span_ref), |p| json!(p))
        }
        "writeTakeSequence" => done(
            write_take_sequence(fs, &dir, &track, &notes_of(at(a, 2)), n(a, 3), meter(4)),
            |p| json!(p),
        ),
        "materialWindow" => {
            let p = at(a, 0);
            let r = material_window(&PlacedExtent {
                length: field(p, "length").unwrap_or(f64::NAN),
                offset: field(p, "offset"),
                loop_start: field(p, "loopStart"),
                loop_end: field(p, "loopEnd"),
            });
            Ok(json!({ "from": r.from, "to": r.to, "loops": r.loops }))
        }
        "resizeToWindow" => {
            let b = at(a, 0);
            let bounds = b.get("bounds").filter(|x| x.is_object()).map(|x| Bounds {
                from: field(x, "from"),
                to: field(x, "to"),
            });
            let before = ResizeBefore {
                kind: kind_of(b),
                from: field(b, "from").unwrap_or(f64::NAN),
                to: field(b, "to"),
                loops: field(b, "loops").unwrap_or(f64::NAN),
                start_beats: field(b, "startBeats").unwrap_or(f64::NAN),
                bounds,
            };
            let t = at(a, 1);
            let to = ResizeTo {
                start_beats: field(t, "startBeats").unwrap_or(f64::NAN),
                end_beats: field(t, "endBeats").unwrap_or(f64::NAN),
                tempo: field(t, "tempo").unwrap_or(f64::NAN),
            };
            let r = resize_to_window(&before, &to);
            Ok(json!({ "startBeats": r.start_beats, "from": r.from, "to": r.to, "loops": r.loops }))
        }
        "planCrossfade" => Ok(
            plan_crossfade(&side(at(a, 0)), &side(at(a, 1)), n(a, 2), n(a, 3)).map_or(Json::Null, |p| {
                let mut o = json!({ "bStart": p.b_start, "bFrom": p.b_from, "length": p.length });
                if let Some(t) = p.a_to {
                    o["aTo"] = json!(t);
                }
                o
            }),
        ),
        "resolveOnsets" => Ok(json!(resolve_onsets(&floats(at(a, 0)), edits_of(a.get(1)).as_ref()))),
        "removeOnset" => Ok(edits_json(&remove_onset(
            edits_of(a.first()).as_ref(),
            &floats(at(a, 1)),
            n(a, 2),
        ))),
        "addOnset" => Ok(edits_json(&add_onset(edits_of(a.first()).as_ref(), n(a, 1)))),
        "moveOnset" => Ok(edits_json(&move_onset(
            edits_of(a.first()).as_ref(),
            &floats(at(a, 1)),
            n(a, 2),
            n(a, 3),
        ))),
        _ => return None,
    })
}
