// graph/view/sounds.rs — the sounds a node draws a pad for, and a live input's numeric channels.
// A reader would get wrong that a rack with no sound is `[]`, present, so its node still draws the "+" a kit starts with.

use super::web::WebRackSound;
use crate::graph::{Channel, GraphModule};
use crate::modules::{ParamValue, RackType};
use crate::resolve::{InstrumentType, resolved_rack, resolved_type};

/// A rack's sounds in note order, then by code point of the symbol; none for a module that is no rack.
pub fn rack_sounds(m: &GraphModule<'_>) -> Option<Vec<WebRackSound>> {
    if resolved_type(Some(m.m)) != InstrumentType::Rack {
        return None;
    }
    let rack = resolved_rack(Some(m.m));
    let slice = rack.rack_type == RackType::Slice;
    let mut out: Vec<WebRackSound> = rack
        .symbols
        .iter()
        .map(|(symbol, s)| WebRackSound {
            symbol: symbol.clone(),
            note: s.note.0,
            sample: s.sample.as_ref().map(|x| x.0.clone()),
            mute: (s.mute == Some(true)).then_some(true),
            source: if slice {
                rack.source.clone().filter(|x| !x.is_empty())
            } else {
                None
            },
            from: if slice { s.from.as_ref().map(|x| x.0) } else { None },
            to: if slice { s.to.as_ref().map(|x| x.0) } else { None },
            clip: if slice {
                None
            } else {
                s.clip.as_ref().map(|x| x.0.clone())
            },
        })
        .collect();
    out.sort_by(|a, b| a.note.cmp(&b.note).then_with(|| a.symbol.cmp(&b.symbol)));
    Some(out)
}

/// An SFZ kit's pads, drawn like a rack's; a mute reads a NUMBER param only (442).
pub fn kit_sounds(m: &GraphModule<'_>) -> Option<Vec<WebRackSound>> {
    let kit = m.m.authored.kit.as_ref().map(|k| &k.0).filter(|k| !k.is_empty())?;
    let params = m.m.authored.params.as_ref();
    let muted = |note: u8| {
        matches!(
            params.and_then(|p| p.get(&format!("Mute {note}"))),
            Some(ParamValue::Number(n)) if *n >= 0.5
        )
    };
    let mut out: Vec<WebRackSound> = kit
        .iter()
        .map(|(symbol, note)| WebRackSound {
            symbol: symbol.clone(),
            note: note.0,
            sample: None,
            clip: None,
            source: None,
            from: None,
            to: None,
            mute: muted(note.0).then_some(true),
        })
        .collect();
    out.sort_by(|a, b| a.note.cmp(&b.note).then_with(|| a.symbol.cmp(&b.symbol)));
    Some(out)
}

/// An input's channels when the file states them as NUMBERS, else none.
pub fn numeric_channels(m: &GraphModule<'_>) -> Option<Vec<u32>> {
    match &m.channel {
        Some(Channel::Numbers(n)) => Some(n.clone()),
        _ => None,
    }
}
