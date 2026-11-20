// clips/audition.rs — the document the engine plays while a timeline is AUDITIONED, and the hole a take is armed into.
// A reader would get wrong that busses and the master stay as they are: the host lane's sends are what make the timeline sound as it does.

use super::doc::{ClipDoc, ClipKind, NestDoc, TimelineLaneDoc};
use super::id::NEST_SEP;
use super::placer::Within;
use super::timelines::place_within;
use super::{Grid, cut_hole};
use crate::doc::{ProjectDoc, TrackDoc};
use song_schema::TimeSig;
use std::collections::HashSet;

/// `Number(s)` is finite: the test `materialRefOf` makes of what follows an `@`.
fn js_finite(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return true;
    }
    let radix = |p: &[&str], r: u32| {
        p.iter()
            .find_map(|p| t.strip_prefix(p))
            .map(|d| !d.is_empty() && u64::from_str_radix(d, r).is_ok())
    };
    if let Some(ok) = radix(&["0x", "0X"], 16)
        .or_else(|| radix(&["0o", "0O"], 8))
        .or_else(|| radix(&["0b", "0B"], 2))
    {
        return ok;
    }
    // Rust reads `inf` and `nan` words that JS does not; neither is finite, so they fall out either way.
    t.parse::<f64>().is_ok_and(f64::is_finite)
}

/// `materialRefOf`: the material a placement id names, split on the LAST `@` and only before a number.
pub fn material_ref_of(nested: &str) -> &str {
    let clip = nested.rsplit_once('>').map_or(nested, |(_, c)| c);
    match clip.rfind('@') {
        Some(at) if at > 0 => {
            let tail = &clip[at + 1..];
            if !tail.is_empty() && js_finite(tail) {
                &clip[..at]
            } else {
                clip
            }
        }
        _ => clip,
    }
}

/// The engine tracks timeline `id` first plays through, both halves of one lane, or none when it is placed nowhere.
pub fn audition_hosts(doc: &ProjectDoc, id: &str) -> Vec<String> {
    // A placement is named by the id prefix up to its own `<id>@<start>` segment, nested or not.
    let placement_of = |c: &ClipDoc| -> Option<String> {
        let segs: Vec<&str> = c.id.split(NEST_SEP).collect();
        let segs = &segs[..segs.len().saturating_sub(1)];
        let at = segs.iter().position(|s| material_ref_of(s) == id)?;
        Some(segs[..=at].join(NEST_SEP))
    };
    let mut first: Option<(String, f64)> = None;
    let mut hosts: Vec<(String, Vec<String>)> = Vec::new();
    for t in &doc.tracks {
        for c in &t.clips {
            let Some(key) = c.nest.as_ref().and_then(|_| placement_of(c)) else {
                continue;
            };
            match hosts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => {
                    if !v.contains(&t.id) {
                        v.push(t.id.clone());
                    }
                }
                None => hosts.push((key.clone(), vec![t.id.clone()])),
            }
            if first.as_ref().is_none_or(|(_, s)| c.start < *s) {
                first = Some((key, c.start));
            }
        }
    }
    first
        .and_then(|(k, _)| hosts.into_iter().find(|(h, _)| *h == k))
        .map(|(_, v)| v)
        .unwrap_or_default()
}

/// A timeline's lane as a track, for whoever reads tracks.
fn lane_track(l: TimelineLaneDoc) -> TrackDoc {
    TrackDoc {
        id: l.id,
        name: l.name,
        role: "regular".into(),
        // Either kind: it plays through whichever lane the timeline is placed on.
        content_type: "audio notes".into(),
        group: String::new(),
        generated: false,
        color: l.color,
        mute: false,
        solo: false,
        highlight: None,
        disabled: l.disabled,
        input_channels: vec![],
        output_channels: vec![],
        monitor: "off".into(),
        input_gain: 0.0,
        clips: l.clips,
        frozen_material: None,
        plugins: vec![],
        sends: vec![],
        input_from: None,
        automation: vec![],
    }
}

/// `doc` playing timeline `id` alone, from beat 0; `doc` itself when there is no such timeline.
pub fn audition_timeline(doc: ProjectDoc, id: &str) -> ProjectDoc {
    let Some(tl) = doc.timelines.iter().flatten().find(|t| t.id == id).cloned() else {
        return doc;
    };
    let source: Vec<ClipDoc> = tl
        .lanes
        .iter()
        .flat_map(|l| {
            l.clips.iter().map(|c| {
                let mut c = c.clone();
                if l.disabled {
                    c.disabled = Some(true);
                }
                c
            })
        })
        .collect();
    let within = Within {
        offset: 0.0,
        end: tl.length,
        gain: 0.0,
        disabled: false,
        prefix: format!("{id}{NEST_SEP}"),
        nest: NestDoc {
            of: format!("{id}@0"),
            ref_: id.to_string(),
            start: 0.0,
            length: tl.length,
            event: None,
        },
    };
    let clips = place_within(source, &within);
    let hosts = audition_hosts(&doc, id);
    let takes = |t: &TrackDoc, c: &ClipDoc| -> bool {
        if c.kind == ClipKind::Midi {
            t.content_type.contains("notes")
        } else {
            t.content_type.contains("audio")
        }
    };
    let own: Vec<&TrackDoc> = doc.tracks.iter().filter(|t| hosts.contains(&t.id)).collect();
    // Each clip on the host half taking its kind, else the first: a lane wired one way still gets the other's material.
    let host_of =
        |c: &ClipDoc| -> Option<String> { own.iter().find(|t| takes(t, c)).or(own.first()).map(|t| t.id.clone()) };
    let host_set: HashSet<&String> = hosts.iter().collect();
    let mut tracks: Vec<TrackDoc> = doc
        .tracks
        .iter()
        .map(|t| {
            if host_set.contains(&t.id) {
                let mut t = t.clone();
                t.mute = false;
                t.solo = false;
                t.clips = clips
                    .iter()
                    .filter(|c| host_of(c).as_deref() == Some(t.id.as_str()))
                    .cloned()
                    .collect();
                t
            } else if t.role != "regular" {
                // A return or the master is left alone: the host's sends reach it.
                t.clone()
            } else {
                let mut t = t.clone();
                t.clips = vec![];
                t.mute = true;
                t.solo = false;
                t
            }
        })
        .collect();
    // Placed nowhere: a bare track, so its audio is heard; its notes have no instrument to play them.
    if hosts.is_empty() {
        tracks.push(lane_track(TimelineLaneDoc {
            id: format!("{id}{NEST_SEP}"),
            name: id.to_string(),
            color: String::new(),
            disabled: false,
            clips,
        }));
    }
    ProjectDoc {
        tracks,
        markers: vec![],
        ..doc
    }
}

/// What the engine is sent while a take is armed: the punch cut out of the lane, and the lane left unmuted.
pub fn with_take_hole(doc: &mut ProjectDoc, track: &str, from: f64, to: f64) {
    let sig = TimeSig::new(doc.time_sig_numerator, doc.time_sig_denominator).or_else(|_| TimeSig::new(4, 4));
    let Ok(sig) = sig else {
        return;
    };
    let grid = Grid::new(sig, doc.tempo);
    doc.record_mute = Some(false);
    for t in doc.tracks.iter_mut().filter(|t| t.id == track) {
        t.clips = cut_hole(&t.clips, from, to, &grid);
    }
}
