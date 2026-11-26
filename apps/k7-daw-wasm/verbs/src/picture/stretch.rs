// stretch.rs — each fade's `chain:` and each shot's `effect:` held to the runs of shaders landing on its own lane.
// A reader would get wrong what a miss does: the fade stays a fade (a dissolve), since a cut would move the arrangement.

use super::doc::{Fade, Track};
use indexmap::IndexMap;
use song::lanes::inputs::Inputs;
use song::routing::{ChainFile, ChainProject, Item};

/// Which node a name on a lane starts at: itself in a graph project, a subchain's first node in a chain project.
pub struct StretchHead<'a> {
    project: Option<&'a ChainProject>,
}

fn picture_lane_of(c: &ChainFile, cp: &ChainProject) -> Option<String> {
    c.steps.iter().flat_map(|s| s.items()).find_map(|i| match i {
        Item::VideoTrack { video_track, .. } => Some(video_track.clone()),
        Item::Track { track, .. } if cp.nodes.get(track).is_some_and(|n| n.picture == Some(true)) => {
            Some(track.clone())
        }
        _ => None,
    })
}

impl<'a> StretchHead<'a> {
    pub fn of(inputs: &'a Inputs) -> StretchHead<'a> {
        StretchHead {
            project: inputs.routing.project.as_ref().filter(|_| inputs.routing.chain_project),
        }
    }

    pub fn head(&self, lane: &str, name: &str) -> String {
        let Some(cp) = self.project else { return name.to_owned() };
        let file = cp
            .chains
            .values()
            .find(|c| picture_lane_of(c, cp).as_deref() == Some(lane));
        let sub = file.and_then(|f| f.subchains.iter().flatten().find(|s| s.id == name));
        sub.and_then(|s| s.steps.iter().flat_map(|st| st.items()).next())
            .map_or_else(|| name.to_owned(), |i| i.name().to_owned())
    }
}

fn check_fade(
    fade: Option<Fade>,
    lane: &str,
    clip: &str,
    lanes_of: &IndexMap<String, Vec<String>>,
    warnings: &mut Vec<String>,
    head: &StretchHead<'_>,
) -> Option<Fade> {
    let fade = fade?;
    let Some(chain) = fade.chain.clone() else {
        return Some(fade);
    };
    let h = head.head(lane, &chain);
    if lanes_of.get(&h).is_some_and(|l| l.iter().any(|x| x == lane)) {
        return Some(Fade { chain: Some(h), ..fade });
    }
    warnings.push(format!(
        "{lane}: '{clip}' names chain '{chain}', which is no run of shaders landing on this lane — it fades as a dissolve"
    ));
    let with_ = if fade.with_ == "over" {
        "dissolve".to_owned()
    } else {
        fade.with_
    };
    Some(Fade {
        over: fade.over,
        with_,
        chain: None,
    })
}

/// Every fade's `chain:` and every `effect:` checked against the stretches landing on its lane.
pub fn resolve_stretches(
    tracks: Vec<Track>,
    lanes_of: &IndexMap<String, Vec<String>>,
    warnings: &mut Vec<String>,
    head: &StretchHead<'_>,
) -> Vec<Track> {
    tracks
        .into_iter()
        .map(|mut t| {
            for c in &mut t.clips {
                c.fade_in = check_fade(c.fade_in.take(), &t.id, &c.id, lanes_of, warnings, head);
                c.fade_out = check_fade(c.fade_out.take(), &t.id, &c.id, lanes_of, warnings, head);
                if let Some(effect) = c.effect.take() {
                    let h = head.head(&t.id, &effect);
                    if lanes_of.get(&h).is_some_and(|l| l.contains(&t.id)) {
                        c.effect = Some(h);
                    } else {
                        warnings.push(format!(
                            "{}: '{}' names effect '{effect}', which is no run of shaders landing on this lane — it plays as it is",
                            t.id, c.id
                        ));
                    }
                }
            }
            t
        })
        .collect()
}
