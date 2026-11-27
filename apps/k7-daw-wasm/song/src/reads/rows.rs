// reads/rows.rs — the lane layout shared by the Timeline and the Minimap, and which lane places a clip.
// A reader would get wrong that a row names its members by INDEX into what the page passed: the page keeps the objects, Rust the order.

use super::graph::node_without_port;
use super::model::{RClip, RTrack};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// One row of the layout, naming what it draws by position in the lists passed in.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(tag = "kind", rename_all = "camelCase"))]
pub enum LayoutRow {
    Group { group: usize },
    /// `index` is the track's position in the original array: clustering reorders the display, not the engine's levels.
    Track { track: usize, index: usize, grouped: bool },
    /// A picture lane has no engine index: `track` is its place among the picture lanes.
    Video { track: usize },
    /// An automation lane of a track: `lane` is its place among that track's.
    Automation { track: usize, lane: usize },
}

struct Unit {
    lanes: Vec<String>,
    rows: Vec<LayoutRow>,
}

/// Flattens tracks into rows, clustering a group under a header at its first member, then threads the picture rows among them.
pub fn project_rows(
    groups: &[String],
    tracks: &[RTrack],
    video: &[String],
    automation: &HashMap<String, usize>,
    order: Option<&[String]>,
) -> Vec<LayoutRow> {
    let group_at: HashMap<&str, usize> = groups.iter().enumerate().map(|(i, g)| (g.as_str(), i)).collect();
    let mut members: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, t) in tracks.iter().enumerate() {
        if group_at.contains_key(t.group.as_str()) {
            members.entry(t.group.as_str()).or_default().push(i);
        }
    }
    let lane_rows = |i: usize, grouped: bool| -> Vec<LayoutRow> {
        let mut v = vec![LayoutRow::Track { track: i, index: i, grouped }];
        for lane in 0..automation.get(&tracks[i].id).copied().unwrap_or(0) {
            v.push(LayoutRow::Automation { track: i, lane });
        }
        v
    };
    let mut units: Vec<Unit> = Vec::new();
    let mut done: HashSet<&str> = HashSet::new();
    for (i, t) in tracks.iter().enumerate() {
        let Some(&g) = group_at.get(t.group.as_str()) else {
            units.push(Unit { lanes: vec![node_without_port(&t.id).to_owned()], rows: lane_rows(i, false) });
            continue;
        };
        if !done.insert(t.group.as_str()) {
            continue;
        }
        let list = members.get(t.group.as_str()).cloned().unwrap_or_default();
        let mut lanes = vec![groups[g].clone()];
        lanes.extend(list.iter().map(|&m| node_without_port(&tracks[m].id).to_owned()));
        let mut rows = vec![LayoutRow::Group { group: g }];
        for &m in &list {
            rows.extend(lane_rows(m, true));
        }
        units.push(Unit { lanes, rows });
    }
    // Picture rows top first: the document is bottom-first, and a row above another is in front of it.
    let pictures: Vec<usize> = (0..video.len()).rev().collect();
    let at: HashMap<&str, usize> = order.unwrap_or(&[]).iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
    let sound: HashSet<&str> = units.iter().flat_map(|u| u.lanes.iter().map(String::as_str)).collect();
    // Some(None) is "before every sound lane", None is "unlisted".
    let anchor_of = |id: &str| -> Option<Option<&str>> {
        let i = *at.get(id)?;
        let order = order?;
        for k in (0..i).rev() {
            if sound.contains(order[k].as_str()) {
                return Some(Some(order[k].as_str()));
            }
        }
        Some(None)
    };
    let mut anchored: HashMap<Option<&str>, Vec<LayoutRow>> = HashMap::new();
    let mut unlisted = Vec::new();
    for p in pictures {
        match anchor_of(&video[p]) {
            None => unlisted.push(LayoutRow::Video { track: p }),
            Some(a) => anchored.entry(a).or_default().push(LayoutRow::Video { track: p }),
        }
    }
    let mut out = anchored.get(&None).cloned().unwrap_or_default();
    for u in units {
        out.extend(u.rows);
        for l in &u.lanes {
            if let Some(v) = anchored.get(&Some(l.as_str())) {
                out.extend(v.iter().cloned());
            }
        }
    }
    out.extend(unlisted);
    out
}

/// The id of the lane placing `clip`, or `""`: the first lane wins, and a frozen lane's authored clips still sit on it.
pub fn track_placing(tracks: &[RTrack], clip: &str) -> String {
    if clip.is_empty() {
        return String::new();
    }
    tracks
        .iter()
        .find(|t| t.clips.iter().any(|c| c.id == clip) || t.frozen_material.iter().flatten().any(|c| c.id == clip))
        .map_or_else(String::new, |t| t.id.clone())
}

/// The index of the placement on a lane playing material `ref_`: a take is named by its material, its placement by `ref@start`.
pub fn placement_of(clips: &[RClip], ref_: &str) -> Option<usize> {
    clips.iter().position(|c| c.id == ref_ || c.name == ref_ || c.pieces.iter().flatten().any(|p| p.name == ref_))
}
