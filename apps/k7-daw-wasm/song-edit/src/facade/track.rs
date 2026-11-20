// track.rs — trackMix's arms: the gain and pan conversions and the placement writes, numbers in and the answer as JSON out.
// A reader would get wrong the infinities: JSON has none, so a number crosses as `{"$":"number"}` and `fnum` reads it back.

use super::args::{at, b, done, fnum, number, s, val_lax};
use super::{Ctx, Out};
use crate::track_mix;
use serde_json::{Value as Json, json};

pub const NAMES: &[(&str, &[&str])] = &[(
    "song-edit/trackMix",
    &[
        "gainToDb",
        "dbToGain",
        "panToLabel",
        "addTrackPlacement",
        "addTrackPlacements",
        "moveTrackPlacement",
        "removeTrackPlacement",
        "moveTrackPlacementsInRange",
        "movePlacementToTrack",
        "setTrackPlacementFade",
        "setTrackPlacementWarp",
        "setTrackPlacementDisabled",
        "setTrackPlacementWindow",
        "resizeTrackPlacement",
    ],
)];

pub fn run(cx: &mut Ctx, _module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let (dir, id) = (s(a, 0), s(a, 1));
    let num = |i: usize| fnum(a, i);
    Some(match f {
        "gainToDb" => Ok(number(track_mix::gain_to_db(num(0)))),
        "dbToGain" => Ok(number(track_mix::db_to_gain(num(0)))),
        "panToLabel" => Ok(json!(track_mix::pan_to_label(num(0)))),
        "addTrackPlacement" => done(track_mix::add_track_placement(fs, &dir, &id, &s(a, 2), &s(a, 3)), |p| {
            json!(p)
        }),
        "addTrackPlacements" => {
            let list: Vec<_> = at(a, 2)
                .as_array()
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .map(val_lax)
                .collect();
            done(track_mix::add_track_placements(fs, &dir, &id, &list), |p| json!(p))
        }
        "moveTrackPlacement" => done(
            track_mix::move_track_placement(fs, &dir, &id, num(2), &s(a, 3), num(4), num(5)),
            |b| json!(b),
        ),
        "removeTrackPlacement" => done(
            track_mix::remove_track_placement(fs, &dir, &id, num(2), num(3), num(4)),
            |b| json!(b),
        ),
        "moveTrackPlacementsInRange" => done(
            track_mix::move_track_placements_in_range(fs, &dir, &id, num(2), num(3), num(4), num(5), num(6)),
            |n| json!(n),
        ),
        "movePlacementToTrack" => done(
            track_mix::move_placement_to_track(fs, &dir, &id, &s(a, 2), num(3), &s(a, 4), num(5), num(6)),
            |b| json!(b),
        ),
        "setTrackPlacementFade" => done(
            track_mix::set_track_placement_fade(fs, &dir, &id, num(2), num(3), num(4), &s(a, 5), num(6)),
            |b| json!(b),
        ),
        "setTrackPlacementWarp" => done(
            track_mix::set_track_placement_warp(fs, &dir, &id, num(2), num(3), num(4), b(a, 5)),
            |b| json!(b),
        ),
        "setTrackPlacementDisabled" => done(
            track_mix::set_track_placement_disabled(fs, &dir, &id, num(2), num(3), num(4), b(a, 5)),
            |b| json!(b),
        ),
        "setTrackPlacementWindow" => {
            let win = (fnum(&[at(a, 5)["from"].clone()], 0), fnum(&[at(a, 5)["to"].clone()], 0));
            done(
                track_mix::set_track_placement_window(fs, &dir, &id, num(2), num(3), num(4), win, num(6)),
                |b| json!(b),
            )
        }
        "resizeTrackPlacement" => {
            done(
                track_mix::resize_track_placement(fs, &dir, &id, num(2), num(3), num(4), num(5), num(6), num(7)),
                |r| {
                    r.map_or(Json::Null, |r| json!({ "ref": r.reference, "material": r.material, "kind": r.kind.name(), "loops": r.loops }))
                },
            )
        }
        _ => return None,
    })
}
