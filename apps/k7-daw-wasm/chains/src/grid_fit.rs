// grid_fit.rs — a card's widget grid made responsive: capped at so many rows (or columns), the overflow folded onto the free side.
// A reader would get wrong that a band is never cut through a control, but a frame is cut and a screen shortened; each tab folds alone.

use crate::panel::{Panel, Tab, Widget, Widgets};
use serde::Deserialize;

/// The one side a grid is held to; the other grows to take what overflows.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GridFit {
    pub axis: Axis,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Rows,
    Cols,
}

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub at: f64,
    pub len: f64,
}

fn along(w: &Widget, axis: Axis) -> Span {
    match axis {
        Axis::Rows => Span {
            at: w.row,
            len: w.rows(),
        },
        Axis::Cols => Span {
            at: w.col,
            len: w.cols(),
        },
    }
}

fn cross_of(w: &Widget, axis: Axis) -> Span {
    along(w, if axis == Axis::Rows { Axis::Cols } else { Axis::Rows })
}

/// The `[start, end)` bands `size` lines fold into, each at most `max` long unless a widget leaves no cut in reach.
pub fn fold_bands(size: f64, max: f64, spans: &[Span]) -> Vec<(f64, f64)> {
    // A cut between line c-1 and c is legal when no widget runs across it.
    let legal = |c: f64| !spans.iter().any(|s| s.at < c && c < s.at + s.len);
    let mut bands = Vec::new();
    let mut start = 0.0;
    while start < size {
        let mut end = size.min(start + max);
        while end > start && end < size && !legal(end) {
            end -= 1.0;
        }
        if end == start {
            end = start + max + 1.0;
            while end < size && !legal(end) {
                end += 1.0;
            }
            end = end.min(size);
        }
        bands.push((start, end));
        start = end;
    }
    bands
}

/// A frame is drawing, not a control: it never holds a cut back, and is redrawn once per band it crosses.
fn is_frame(w: &Widget) -> bool {
    w.kind == "group"
}

/// A screen draws a picture scaled to its box: it never holds a cut back either, and ends at its band's edge.
fn is_elastic(w: &Widget) -> bool {
    w.kind == "lcdscreen" || w.kind == "waveform"
}

/// A fold: the bands, how wide each is across, and where each starts.
#[derive(Clone, Debug)]
struct Fold {
    bands: Vec<(f64, f64)>,
    widths: Vec<f64>,
    offsets: Vec<f64>,
}

impl Fold {
    fn across(&self) -> f64 {
        self.widths.iter().sum()
    }
}

fn band_of(bands: &[(f64, f64)], at: f64) -> usize {
    bands.iter().position(|(s, e)| at >= *s && at < *e).unwrap_or(0)
}

/// `bands` sized to `widgets`: each band as wide as the furthest control or screen in it, a frame's width not counting.
fn sized(bands: Vec<(f64, f64)>, widgets: &[&Widget], axis: Axis) -> Fold {
    let mut widths = vec![1.0_f64; bands.len()];
    for w in widgets.iter().filter(|w| !is_frame(w)) {
        let b = band_of(&bands, along(w, axis).at);
        let c = cross_of(w, axis);
        widths[b] = widths[b].max(c.at + c.len);
    }
    let offsets = (0..widths.len()).map(|i| widths[..i].iter().sum()).collect();
    Fold { bands, widths, offsets }
}

/// Where a fold puts a widget: one piece per band it covers (a frame may cover several, anything else one).
fn place_in(key: &str, w: &Widget, fold: &Fold, axis: Axis) -> Vec<(String, Widget)> {
    let Fold { bands, widths, offsets } = fold;
    let Span { at, len } = along(w, axis);
    let cross = cross_of(w, axis);
    let shift = |b: usize, from: f64, size: f64, cross_len: f64| -> Widget {
        let mut out = w.clone();
        match axis {
            Axis::Rows => {
                out.row = from - bands[b].0;
                out.row_span = Some(size);
                out.col = cross.at + offsets[b];
                out.col_span = Some(cross_len);
            }
            Axis::Cols => {
                out.col = from - bands[b].0;
                out.col_span = Some(size);
                out.row = cross.at + offsets[b];
                out.row_span = Some(cross_len);
            }
        }
        out
    };
    if !is_frame(w) {
        let b = band_of(bands, at);
        let fits = if is_elastic(w) { len.min(bands[b].1 - at) } else { len };
        return vec![(key.to_owned(), shift(b, at, fits, cross.len))];
    }
    // A frame is cut at a band's edge and at its width: one piece per band, the label on the first only.
    let mut out: Vec<(String, Widget)> = Vec::new();
    for (b, (s, e)) in bands.iter().enumerate() {
        let from = at.max(*s);
        let to = (at + len).min(*e);
        let wide = cross.len.min(widths[b] - cross.at);
        if to <= from || wide <= 0.0 {
            continue;
        }
        let piece = shift(b, from, to - from, wide);
        if out.is_empty() {
            out.push((key.to_owned(), piece));
        } else {
            out.push((format!("{key}~{b}"), Widget { label: None, ..piece }));
        }
    }
    out
}

fn fold_widgets(ws: &Widgets, f: &Fold, axis: Axis) -> Widgets {
    ws.iter().flat_map(|(k, w)| place_in(k, w, f, axis)).collect()
}

/// `ui` folded to `fit`, or `None` when it already fits or there is nowhere legal to fold it.
pub fn fit_panel(ui: &Panel, fit: GridFit) -> Option<Panel> {
    let GridFit { axis, max } = fit;
    let size = if axis == Axis::Rows { ui.rows } else { ui.cols };
    if max.is_nan() || max < 1.0 || size <= max {
        return None;
    }
    let spans_of = |ws: &[&Widget]| -> Vec<Span> {
        ws.iter()
            .filter(|w| !is_frame(w) && !is_elastic(w))
            .map(|w| along(w, axis))
            .collect()
    };
    let common: Vec<&Widget> = ui.widgets.values().collect();
    let tabs: Vec<Vec<&Widget>> = ui.tabs.iter().flatten().map(|t| t.widgets.values().collect()).collect();
    // Each tab folds on its own, the widgets outside the tabs with it: only one tab is ever drawn.
    let per_tab: Vec<Fold> = tabs
        .iter()
        .map(|ws| {
            let drawn: Vec<&Widget> = common.iter().chain(ws.iter()).copied().collect();
            sized(fold_bands(size, max, &spans_of(&drawn)), &drawn, axis)
        })
        .collect();
    // …which holds only while the shared widgets land in the same place in every tab's fold; otherwise one fold for all.
    let everything: Vec<&Widget> = common.iter().copied().chain(tabs.iter().flatten().copied()).collect();
    let pooled = sized(fold_bands(size, max, &spans_of(&everything)), &everything, axis);
    let place_key =
        |f: &Fold| -> Vec<Vec<(String, Widget)>> { ui.widgets.iter().map(|(k, w)| place_in(k, w, f, axis)).collect() };
    let shared_agree = !per_tab.is_empty() && {
        let first = place_key(&per_tab[0]);
        per_tab.iter().all(|f| place_key(f) == first)
    };
    let tab_folds: Vec<Fold> = if shared_agree {
        per_tab.clone()
    } else {
        per_tab.iter().map(|_| pooled.clone()).collect()
    };
    let common_fold = if shared_agree { per_tab[0].clone() } else { pooled };
    let all: Vec<&Fold> = std::iter::once(&common_fold).chain(tab_folds.iter()).collect();
    if all.iter().all(|f| f.bands.len() < 2) {
        return None;
    }
    let long = all
        .iter()
        .flat_map(|f| f.bands.iter().map(|(s, e)| e - s))
        .fold(f64::MIN, f64::max);
    let wide = all.iter().map(|f| f.across()).fold(f64::MIN, f64::max);
    let mut out = ui.clone();
    match axis {
        Axis::Rows => (out.rows, out.cols) = (long, wide),
        Axis::Cols => (out.cols, out.rows) = (long, wide),
    }
    out.widgets = fold_widgets(&ui.widgets, &common_fold, axis);
    if let Some(ts) = &ui.tabs {
        out.tabs = Some(
            ts.iter()
                .enumerate()
                .map(|(i, t)| Tab {
                    widgets: fold_widgets(&t.widgets, &tab_folds[i], axis),
                    // Each tab's own width, rows held: a card sized to the open tab, not to the widest of them.
                    cols: if axis == Axis::Rows {
                        Some(tab_folds[i].across())
                    } else {
                        t.cols
                    },
                    ..t.clone()
                })
                .collect(),
        );
    }
    Some(out)
}

#[cfg(test)]
mod tests;
