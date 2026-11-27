// anchor_right.rs — a module row's trailing MIXER, lifted out so the host pins it to the pane's right edge.
// The row keeps every other card and cable; the mixer becomes a graph of its own, one BARE card at the row's height.
// A reader would get wrong that the cable INTO the mixer is dropped, not redrawn: a pinned card no longer sits after its feeder.

use crate::geom::node_size;
use crate::node::Laid;
use serde::Serialize;

/// A row without its last card, and that card alone.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SplitMixer {
    pub row: Laid,
    pub mixer: Laid,
}

/// The row without its last card, and that card alone, or none when the row does not END on a lane's mixer.
pub fn split_trailing_mixer(row: &Laid) -> Option<SplitMixer> {
    let n = row.nodes.len();
    let last = row.nodes.last()?;
    let before = n.checked_sub(2).and_then(|i| row.nodes.get(i))?;
    // A mixer wears its lane's face; one with nothing before it is the whole row, and stays in it.
    last.track_face.as_ref().filter(|f| !f.is_empty())?;
    // `module_row` starts its first card one margin in, and ends the row the same margin past its last.
    let margin = row.nodes.first()?.x.unwrap_or(0.0) - row.left;
    // Sized by its panel alone, the width a bare card needs whatever its header held.
    let (w, _) = node_size(last.ui.as_ref(), false);
    let mut kept = row.clone();
    kept.nodes.pop();
    kept.edges.retain(|e| e.source != last.id && e.target != last.id);
    kept.width = before.x.unwrap_or(0.0) + before.w + margin;
    // No margin either side: the card is flush with the pane's right edge, and the row keeps its own gap.
    let mut card = last.clone();
    card.x = Some(row.left);
    card.w = w;
    card.bare = Some(true);
    let mixer = Laid {
        nodes: vec![card],
        // Only the automations landing on it: no cable is drawn from a curve not here, but its knobs glow.
        edges: row
            .edges
            .iter()
            .filter(|e| e.automation.is_some() && e.target == last.id)
            .cloned()
            .collect(),
        width: row.left + w,
        ..row.clone()
    };
    Some(SplitMixer { row: kept, mixer })
}

#[cfg(test)]
mod tests;
