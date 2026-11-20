// pictures/join.rs — nodeProject.ts lines 218 to 244: the placed shots, the merge, the lanes' sounds and the pictures' own, in that order.
// A reader would get wrong that this makes no track: 2h places the clips and turns every finding here into its own list (decision 526).

use super::{
    ClipSounds, LaneShots, SoundFault, SoundLaneIn, Sounder, Spend, picture_lane_sounds, timeline_shots,
    video_sound_lanes, weigh, with_timeline_shots,
};
use crate::clips::{ClipDoc, Placer};
use crate::files::Finding;
use crate::lanes::inputs::Inputs;
use crate::lanes::{LaneKind, Lanes};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use std::collections::HashSet;

/// What the pictures give a compile: the two maps of clips, and the faults and findings that stop it.
pub struct PictureSounds {
    pub video_sounds: IndexMap<String, Vec<ClipDoc>>,
    pub picture_sounds: IndexMap<String, Vec<ClipDoc>>,
    pub faults: Vec<SoundFault>,
    pub findings: Vec<Finding>,
    pub capped: bool,
    pub spent: usize,
}

/// The shots every sound lane's timelines place, lane by lane in file order; a lane's file names where a stop was (518).
fn placed_in<'t, 'a: 't>(
    lanes: &'t Lanes<'t>,
    placer: &mut Placer<'a>,
    spend: &mut Spend,
    file_of: &dyn Fn(&str) -> String,
) -> LaneShots<'t> {
    let mut out = LaneShots::new();
    for (&name, lane) in &lanes.sound {
        if lane.kind == LaneKind::Group || lane.disabled {
            continue;
        }
        let file = file_of(name);
        if !spend.charge(weigh(name)) {
            spend.limit(&file);
            break;
        }
        if lane.events.is_empty() {
            continue;
        }
        spend.in_file(&file);
        timeline_shots(lane.events, placer, spend, &mut out);
        if spend.capped() {
            break;
        }
    }
    out
}

/// The shots the timelines placed on the sound lanes put in the pictures, by picture lane name.
pub fn placed_shots_of<'t, 'a: 't>(lanes: &'t Lanes<'t>, placer: &mut Placer<'a>, spend: &mut Spend) -> LaneShots<'t> {
    placed_in(lanes, placer, spend, &|name| format!("nodes/{name}.k7x"))
}

/// The file a lane is read from, for a finding.
fn lane_file(inputs: &Inputs, name: &str) -> String {
    inputs
        .local
        .modules
        .get(name)
        .map_or_else(|| format!("nodes/{name}.k7x"), |m| m.own_label())
}

/// A picture lane's sound and the sound of every lane naming one, in nodeProject's order (526).
pub fn picture_sounds(
    source: &dyn Source,
    inputs: &Inputs,
    lanes: &Lanes<'_>,
    placer: &mut Placer<'_>,
) -> Result<PictureSounds, SourceError> {
    let mut spend = Spend::new();
    let placed = placed_in(lanes, placer, &mut spend, &|name| lane_file(inputs, name));
    let pictures = with_timeline_shots(
        lanes.picture.iter().map(|(n, l)| (*n, l.events)),
        placed,
        &mut spend,
        &|name| lane_file(inputs, name),
    );
    let sound_lanes: Vec<SoundLaneIn<'_>> = lanes
        .sound
        .iter()
        .filter(|(_, l)| l.video_source.is_some())
        .map(|(&name, l)| SoundLaneIn {
            name,
            events: l.events.len(),
            video_source: l.video_source,
            file: lane_file(inputs, name),
        })
        .collect();
    let wired: HashSet<&str> = inputs
        .chains
        .values()
        .filter(|c| c.from_port.as_deref() == Some("audio"))
        .filter_map(|c| c.from.as_deref())
        .collect();
    let bps = placer.grid().tempo / 60.0;
    let sounds = ClipSounds::of(&inputs.clips);
    let (video_sounds, picture_sounds) = {
        let mut sounder = Sounder::new(source, &inputs.project, bps, &sounds, &mut spend).with_rule(inputs.rule);
        let video = video_sound_lanes(&mut sounder, &sound_lanes, &pictures)?;
        let own = picture_lane_sounds(&mut sounder, &sound_lanes, &pictures, &|l| wired.contains(l), &|l| {
            lane_file(inputs, l)
        })?;
        (video, own)
    };
    let (capped, spent) = (spend.capped(), spend.spent());
    let (findings, faults) = spend.into_parts();
    Ok(PictureSounds {
        video_sounds,
        picture_sounds,
        faults,
        findings,
        capped,
        spent,
    })
}
