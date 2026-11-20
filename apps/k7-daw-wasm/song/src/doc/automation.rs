// doc/automation.rs — the curves bound to plugins, baked into each owner's automation once every track exists.
// A reader would get wrong that note-ons are laid once per track and kept: a thousand curves on one lane read them once.

use super::notes::Notes;
use super::plugins::RawAutomation;
use super::{AutomationDoc, TrackDoc, text_weight};
use crate::cache::Cache;
use crate::clips::{Budget, ClipKind};
use crate::curves::{CurveBaker, MidiWindow, read_curves};
use crate::lanes::inputs::Inputs;
use crate::lanes::ownership::Ownership;
use crate::source::{Source, SourceError};
use song_schema::TimeSig;
use std::collections::HashMap;

/// What the bake spent, for the gate.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BakeSpent {
    pub baker: usize,
}

/// The windows of a track's MIDI clips, their note times in one flat vector (decision 328).
struct Notes0 {
    times: Vec<f64>,
    windows: Vec<(f64, f64, f64, f64, usize, usize)>,
}

fn midi_of(t: &TrackDoc, budget: &mut Budget) -> Option<Notes0> {
    let n: usize = t
        .clips
        .iter()
        .filter(|c| c.kind == ClipKind::Midi)
        .map(|c| 1 + c.notes.len())
        .sum();
    if !budget.charge(n) {
        return None;
    }
    let mut out = Notes0 {
        times: Vec::new(),
        windows: Vec::new(),
    };
    for c in t.clips.iter().filter(|c| c.kind == ClipKind::Midi) {
        let from = out.times.len();
        out.times.extend(c.notes.iter().map(|n| n.time));
        out.windows
            .push((c.start, c.length, c.loop_start, c.loop_end, from, out.times.len()));
    }
    Some(out)
}

/// Bakes `raws` into the tracks they drive. The baker's ERRORs go to `notes` and stop nothing (decision 304).
#[allow(clippy::too_many_arguments)]
pub(super) fn bake_automations(
    source: &dyn Source,
    cache: &mut Cache,
    inputs: &Inputs,
    own: &Ownership,
    raws: &[RawAutomation<'_>],
    tracks: &mut [TrackDoc],
    sig: TimeSig,
    tempo: f64,
    notes: &mut Notes,
    budget: &mut Budget,
) -> Result<BakeSpent, SourceError> {
    if raws.is_empty() {
        return Ok(BakeSpent::default());
    }
    let curves = read_curves(source, cache, &inputs.project)?;
    for f in &curves.findings {
        notes.push(f.clone());
    }
    let clips: usize = tracks.iter().map(|t| 1 + t.clips.len()).sum();
    if !budget.charge(clips + inputs.chains.len()) {
        return Ok(BakeSpent::default());
    }
    let song_beats = tracks
        .iter()
        .flat_map(|t| &t.clips)
        .fold(0.0_f64, |m, c| m.max(c.start + c.length));
    let by_id: HashMap<&str, usize> = tracks.iter().enumerate().map(|(i, t)| (t.id.as_str(), i)).collect();
    let mut lane_of_module: HashMap<&str, &str> = HashMap::new();
    for (name, c) in &inputs.chains {
        if let Some(owner) = own.owner_of(name) {
            for s in &c.use_steps {
                lane_of_module.insert(&s.module, owner);
            }
        }
    }
    let mut baker = CurveBaker::new(&curves.curves, sig, tempo, song_beats);
    let mut memo: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut baked: Vec<(usize, AutomationDoc)> = Vec::new();
    for ra in raws {
        let Some(&at) = by_id.get(ra.owner) else { continue };
        let from = lane_of_module.get(ra.trigger).copied().unwrap_or(ra.owner);
        let events_at = by_id.get(from).copied();
        let empty: Vec<f64> = Vec::new();
        let events: &Vec<f64> = match events_at {
            None => &empty,
            Some(i) => {
                if let std::collections::hash_map::Entry::Vacant(slot) = memo.entry(i) {
                    let laid = match midi_of(&tracks[i], budget) {
                        Some(n) => {
                            let ws: Vec<MidiWindow> = n
                                .windows
                                .iter()
                                .map(|&(start, length, loop_start, loop_end, a, b)| MidiWindow {
                                    start,
                                    length,
                                    loop_start,
                                    loop_end,
                                    note_times: &n.times[a..b],
                                })
                                .collect();
                            baker.note_ons(&ws)
                        }
                        None => Vec::new(),
                    };
                    slot.insert(laid);
                }
                &memo[&i]
            }
        };
        let Some(curve) = baker.bake(ra.curve, events) else {
            continue;
        };
        if !budget.charge(1 + text_weight(ra.param) + (ra.chain.len() + ra.module.len() + 1) / 16) {
            break;
        }
        let plugin_id = format!("{}:{}", ra.chain, ra.module);
        baked.push((
            at,
            AutomationDoc {
                param: ra.param.to_owned(),
                plugin_id: Some(plugin_id),
                bipolar: Some(curve.bipolar),
                points: curve.points,
            },
        ));
    }
    let (findings, spent) = baker.finish();
    for f in findings {
        notes.push(f);
    }
    for (at, a) in baked {
        tracks[at].automation.push(a);
    }
    Ok(BakeSpent { baker: spent })
}
