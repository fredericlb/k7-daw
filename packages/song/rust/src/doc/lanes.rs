// doc/lanes.rs — the engine tracks one lane compiles to (its halves) and the tracks of a picture lane's own sound.
// A reader would get wrong that halves are indexed once: a lane's port lookups are slots, never a search per event (decision 309).

use super::{TrackDoc, text_weight};
use crate::clips::{ClipDoc, Half};
use crate::lanes::Material;
use crate::routing::Chain;
use indexmap::IndexMap;
use std::collections::HashMap;

/// The picture lanes' sound as the clips 2i derives for one pass (decisions 333, 526).
#[derive(Clone, Debug, Default)]
pub struct Pictures {
    /// The clips of a sounding lane, laid on its audio half.
    pub video_sounds: IndexMap<String, Vec<ClipDoc>>,
    /// The clips of a picture lane's own sound.
    pub picture_sounds: IndexMap<String, Vec<ClipDoc>>,
}

/// The chains leaving each lane, by lane, in chain order.
pub(super) struct Halves<'c> {
    by_lane: HashMap<&'c str, Vec<Half<'c>>>,
    audio: HashMap<&'c str, Vec<Half<'c>>>,
}

impl<'c> Halves<'c> {
    /// Built once from the chains (decision 309).
    pub fn of(chains: &'c IndexMap<String, Chain>) -> Halves<'c> {
        let mut by_lane: HashMap<&str, Vec<Half>> = HashMap::new();
        let mut audio: HashMap<&str, Vec<Half>> = HashMap::new();
        for (name, c) in chains {
            let Some(lane) = c.from.as_deref() else { continue };
            let half = Half {
                id: name,
                port: c.from_port.as_deref(),
            };
            by_lane.entry(lane).or_default().push(half);
            if c.from_port.as_deref() == Some("audio") {
                audio.entry(lane).or_default().push(half);
            }
        }
        Halves { by_lane, audio }
    }

    /// The halves of a lane; a lane wired to nothing is one half of its own name.
    pub fn of_lane(&self, lane: &'c str) -> Vec<Half<'c>> {
        self.by_lane
            .get(lane)
            .cloned()
            .unwrap_or_else(|| vec![Half { id: lane, port: None }])
    }

    /// The chains leaving a picture lane's `#audio`.
    pub fn audio_of(&self, lane: &str) -> &[Half<'c>] {
        self.audio.get(lane).map_or(&[], Vec::as_slice)
    }
}

/// Which half takes each material: the first with that port, else the first with none.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HalfIdx {
    audio: Option<usize>,
    midi: Option<usize>,
    missing: Option<usize>,
    portless: Option<usize>,
}

impl HalfIdx {
    pub fn of(halves: &[Half<'_>]) -> HalfIdx {
        let mut idx = HalfIdx::default();
        for (i, h) in halves.iter().enumerate() {
            let slot = match h.port {
                None => &mut idx.portless,
                Some("audio") => &mut idx.audio,
                Some("midi") => &mut idx.midi,
                Some("missing") => &mut idx.missing,
                Some(_) => continue,
            };
            slot.get_or_insert(i);
        }
        idx
    }

    /// The half an event lands on, by its material.
    pub fn for_material(&self, m: Material) -> Option<usize> {
        match m {
            Material::Audio => self.audio,
            Material::Midi => self.midi,
            Material::Missing => self.missing,
        }
        .or(self.portless)
    }
}

/// The engine's word for what a track holds, from the materials its events carry (decision 310).
pub(super) fn content_type(port: Option<&str>, generated: bool, midi: bool, other: bool) -> &'static str {
    match port {
        Some("midi") => "notes",
        Some(_) => "audio",
        None if generated => "audio notes",
        None => match (midi, other) {
            (true, true) => "audio notes",
            (true, false) => "notes",
            _ => "audio",
        },
    }
}

/// `main#audio`: the track a picture lane's own sound plays on.
pub(super) fn lane_sound_ref(lane: &str) -> String {
    format!("{lane}#audio")
}

/// What a picture lane's sound track weighs before it is made: its id, name, group and colour.
pub(super) fn picture_cost(lane: &str, color: &str) -> usize {
    3 + 2 * text_weight(lane) + text_weight(color)
}

/// What a track weighs before it is made: its strings (id and name both), its channels and the render that stands in for its clips.
pub(super) fn track_cost(strings: &[&str], channels: usize, render: Option<&str>) -> usize {
    1 + strings.iter().map(|s| text_weight(s)).sum::<usize>() + channels + render.map_or(0, |r| 3 + text_weight(r))
}

/// A picture lane's sound track(s): one per chain leaving its `#audio`, else one of its own (decision 333).
#[allow(clippy::too_many_arguments)]
pub(super) fn picture_sound_track(
    lane: &str,
    half: Option<&Half<'_>>,
    first: bool,
    color: &str,
    muted: bool,
    solo: bool,
    clips: Vec<ClipDoc>,
) -> TrackDoc {
    let own = lane_sound_ref(lane);
    let id = if first {
        own.clone()
    } else {
        half.map_or_else(|| own.clone(), |h| h.id.to_owned())
    };
    let mut t = TrackDoc::new(&id);
    t.color = color.to_owned();
    t.mute = muted;
    t.solo = solo;
    t.clips = clips;
    t
}
