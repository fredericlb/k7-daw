// files/timeline.rs — a timelines/ file: lanes of events, placed whole as one event on another lane; after timelineSchema.ts.
// A reader would get wrong that its lanes are not the project's: no module, no plugin, no routing.

use super::Check;
use super::placement::Placement;
use super::scalar::{LaneName, LengthTimecode};
use k7x::PathSeg;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A timeline's own lane, or its picture lane: a name and its events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineLane {
    pub lane: LaneName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<Placement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<LengthTimecode>,
    pub lanes: Vec<TimelineLane>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pictures: Option<Vec<TimelineLane>>,
}

fn path(parts: &[&str], index: usize, rest: &[&str]) -> Vec<PathSeg> {
    let key = |s: &&str| PathSeg::Key((*s).to_owned());
    let mut out: Vec<PathSeg> = parts.iter().map(key).collect();
    out.push(PathSeg::Index(index));
    out.extend(rest.iter().map(key));
    out
}

impl Check for Timeline {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let groups = [("lanes", Some(&self.lanes)), ("pictures", self.pictures.as_ref())];
        for (name, lanes) in groups {
            for (i, lane) in lanes.into_iter().flatten().enumerate() {
                for (j, event) in lane.events.iter().flatten().enumerate() {
                    if let Some((mut at, message)) = event.check() {
                        let mut full = path(&[name], i, &["events"]);
                        full.push(PathSeg::Index(j));
                        full.append(&mut at);
                        return Some((full, message));
                    }
                }
            }
        }
        // Two lanes of one name, sound or picture, would be one address for two lists of events.
        let mut seen: HashSet<&str> = HashSet::new();
        for (name, lanes) in groups {
            for (i, lane) in lanes.into_iter().flatten().enumerate() {
                if !seen.insert(lane.lane.0.as_str()) {
                    return Some((
                        path(&[name], i, &["lane"]),
                        format!("lane '{}' is named twice", lane.lane.0),
                    ));
                }
            }
        }
        None
    }
}
