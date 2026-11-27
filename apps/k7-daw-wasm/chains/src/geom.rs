// geom.rs — the measurements the build decides with: card sizes, the grid's cells, the sockets a card shows, a lane's clip list.
// A reader would get wrong that only the sizes live here: where a socket is DRAWN (`jackPoint`, `cablePath`) stays in the page.

use crate::node::{Jack, NodeClip, Port, Signal};
use crate::panel::Panel;
use std::collections::HashSet;

pub const MAIN_OUT: &str = "OUT";
pub const MIX_OUT: &str = "MIX";
pub const RENDER_OUT: &str = "RENDER";
pub const VIDEO_OUT: &str = "VIDEO_OUT";

pub const NODE_W: f64 = 160.0;
pub const NODE_H: f64 = 70.0;
pub const NODE_MIN_W: f64 = 72.0;
pub const CELL: f64 = 36.0;
pub const UI_PAD: f64 = 6.0;
pub const UI_HEADER: f64 = 38.0;
pub const UI_TOP_GAP: f64 = 4.0;
pub const UI_CONTENT_TOP: f64 = UI_HEADER + UI_TOP_GAP;
pub const TAB_COL_W: f64 = 66.0;
pub const TAB_H: f64 = 20.0;
pub const TAB_MENU_H: f64 = 26.0;
pub const ROW_GAP: f64 = 8.0;
pub const UI_EDGE: f64 = 12.0;
pub const COL_GAP: f64 = 6.0;
pub const TAB_COL_MAX_ROWS: f64 = 4.0;
pub const FOOTER_H: f64 = 20.0;
pub const CLIP_LIST_MAX: usize = 6;

const NOTE_CHAR_W: f64 = 6.6;
const NOTE_LINE_H: f64 = 17.0;
const NOTE_PAD: f64 = 10.0;
const NOTE_MIN_W: f64 = 140.0;
const NOTE_MAX_W: f64 = 320.0;
const CLIP_ROW_H: f64 = 16.0;
const CLIP_LIST_PAD: f64 = 7.0;
const CLIP_TIMELINE_H: f64 = 14.0;
const CLIP_MIN_W: f64 = 0.02;

/// The height `rows` rows take on a card, the gaps between them included.
pub fn rows_height(rows: f64) -> f64 {
    rows * CELL + (rows - 1.0).max(0.0) * ROW_GAP
}

/// The width `cols` columns take on a card, the gaps between them included.
pub fn cols_width(cols: f64) -> f64 {
    cols * CELL + (cols - 1.0).max(0.0) * COL_GAP
}

/// The tab column's width: `TAB_COL_W` when the card draws its tabs as a column, else nothing.
pub fn column_w(ui: Option<&Panel>) -> f64 {
    match ui {
        Some(p) if p.has_tabs() && p.tabs_as.as_deref() != Some("dropdown") => TAB_COL_W,
        _ => 0.0,
    }
}

/// …and the tab menu's height, when a narrow card offers its tabs across its top instead.
pub fn menu_h(ui: Option<&Panel>) -> f64 {
    match ui {
        Some(p) if p.has_tabs() && p.tabs_as.as_deref() == Some("dropdown") => TAB_MENU_H,
        _ => 0.0,
    }
}

/// How big a node must be to hold `ui` and its tab column; with `footer`, the bottom bar under them.
pub fn node_size(ui: Option<&Panel>, footer: bool) -> (f64, f64) {
    let (w, h) = panel_size(ui);
    if footer { (w, h + FOOTER_H) } else { (w, h) }
}

fn panel_size(ui: Option<&Panel>) -> (f64, f64) {
    let col_w = column_w(ui);
    let rows = ui.map_or(0.0, |p| p.rows);
    let tabs_h = if col_w > 0.0 {
        let n = ui.and_then(|p| p.tabs.as_ref()).map_or(0, Vec::len) as f64;
        (n * TAB_H).min(rows_height(rows).max(rows_height(TAB_COL_MAX_ROWS)))
    } else {
        0.0
    };
    let menu = menu_h(ui);
    // A panel with no columns or no rows (or a NaN of either) is no grid at all.
    let Some(p) = ui.filter(|p| p.cols != 0.0 && p.rows != 0.0 && !p.cols.is_nan() && !p.rows.is_nan()) else {
        return (NODE_W + col_w, NODE_H + menu);
    };
    (
        NODE_MIN_W.max(col_w + UI_PAD * 2.0 + cols_width(p.cols)),
        NODE_H.max(UI_CONTENT_TOP + menu + rows_height(p.rows).max(tabs_h) + UI_EDGE),
    )
}

/// The length of a text as JavaScript counts it: in UTF-16 units.
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// How big a note has to be to show its text.
pub fn note_size(text: &str) -> (f64, f64) {
    let lines: Vec<usize> = text.split('\n').map(js_len).collect();
    let widest = lines.iter().copied().max().unwrap_or(0) as f64;
    let w = NOTE_MIN_W.max(NOTE_MAX_W.min(widest * NOTE_CHAR_W + NOTE_PAD * 2.0));
    let per_line = 1.0_f64.max(((w - NOTE_PAD * 2.0) / NOTE_CHAR_W).floor());
    let drawn: f64 = lines.iter().map(|l| 1.0_f64.max((*l as f64 / per_line).ceil())).sum();
    (w, NODE_H.max(drawn * NOTE_LINE_H + NOTE_PAD * 2.0))
}

/// The clip list of a lane's card and the height it adds; `None` when the lane holds none.
pub struct ClipList {
    pub clips: Vec<NodeClip>,
    pub more: Option<f64>,
    pub w: f64,
    pub h: f64,
}

/// A lane's clips against the shared `span`, and the card's size once the list is drawn under its grid.
pub fn clips_of(all: Option<&Vec<crate::graph::TrackClip>>, base: (f64, f64), span: f64) -> Option<ClipList> {
    let all = all.filter(|a| !a.is_empty())?;
    let clips: Vec<NodeClip> = all
        .iter()
        .map(|c| {
            let at = if span > 0.0 {
                (c.start / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let w = match c.len {
                Some(len) if span > 0.0 && len != 0.0 => CLIP_MIN_W.max(len / span),
                _ => CLIP_MIN_W,
            };
            let w = w.min(1.0 - at);
            NodeClip {
                name: c.name.clone(),
                start: c.start,
                tc: c.tc.clone(),
                len: c.len,
                kind: c.kind.clone(),
                at,
                w: if w == 0.0 || w.is_nan() { CLIP_MIN_W } else { w },
            }
        })
        .collect();
    let more = all.len().saturating_sub(CLIP_LIST_MAX);
    let rows = all.len().min(CLIP_LIST_MAX) + usize::from(more > 0);
    Some(ClipList {
        clips,
        more: (more > 0).then_some(more as f64),
        w: base.0,
        h: base.1 + CLIP_LIST_PAD + CLIP_TIMELINE_H + rows as f64 * CLIP_ROW_H,
    })
}

/// The sockets a node shows: inputs reveal themselves, outputs show at once.
pub fn jacks_for(
    ports: Option<&[Port]>,
    fed: &HashSet<String>,
    fallback: Option<Signal>,
    grow: bool,
    spare: bool,
    bare_on: Option<&str>,
    bare: usize,
) -> Vec<Jack> {
    let plain = |kind, free| Jack { name: None, kind, free };
    let Some(ports) = ports.filter(|p| !p.is_empty()) else {
        let kind = fallback.unwrap_or(Signal::Audio);
        if spare && !fed.is_empty() {
            return vec![plain(kind, false), plain(kind, true)];
        }
        if bare > 1 {
            return (0..bare).map(|_| plain(kind, false)).collect();
        }
        return vec![plain(kind, fed.is_empty())];
    };
    let takes_bare = |name: &str, i: usize| bare_on.map_or(i == 0, |b| name == b);
    let is_fed = |p: &Port, i: usize| fed.contains(&p.name) || (takes_bare(&p.name, i) && fed.contains(""));
    let named = |p: &Port, free| Jack {
        name: Some(p.name.clone()),
        kind: p.kind,
        free,
    };
    if !grow {
        let mut out = Vec::new();
        for (i, p) in ports.iter().enumerate() {
            let jack = named(p, !is_fed(p, i));
            let free = jack.free;
            out.push(jack);
            if spare && !free {
                // The spare twin carries the same address as the cable already on it.
                let bare_cable = takes_bare(&p.name, i) && fed.contains("");
                out.push(Jack {
                    name: (!bare_cable).then(|| p.name.clone()),
                    kind: p.kind,
                    free: true,
                });
            }
        }
        return out;
    }
    let mandatory = ports.iter().filter(|p| p.mandatory != Some(false)).count();
    let mut last_fed: i64 = -1;
    for (i, p) in ports.iter().enumerate() {
        if is_fed(p, i) {
            last_fed = i as i64;
        }
    }
    let shown = ports.len().min(1.max(mandatory).max((last_fed + 2) as usize));
    ports
        .iter()
        .take(shown)
        .enumerate()
        .map(|(i, p)| named(p, !is_fed(p, i)))
        .collect()
}

#[cfg(test)]
mod tests;
