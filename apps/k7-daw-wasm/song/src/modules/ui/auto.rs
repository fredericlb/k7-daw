// auto.rs — a panel PROPOSED from a parameter list alone: guess each name's family, and lay the families out as tabs, in both shapes.
// A reader would get wrong that the table's ORDER is the algorithm: reordering it rehomes parameters. Nothing here is authoritative: apps/k7-daw-frontend/packages/ui-module-builder/docs/autoLayout.md.

use super::geometry::LAYOUT_MAX;
use super::rx::Rx;
use crate::doc::{INPUT_VOLUME_PARAM, LevelSide, MIX_PARAM, OUTPUT_VOLUME_PARAM, own_level_side, own_mix_side};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

/// How many cells a knob takes in an auto-organised panel, and only there.
pub const KNOB_SPAN: usize = 2;

/// `@base`'s page and the panel's first tab, holding nothing else.
pub const GENERAL_PAGE: &str = "General";
const TRACK_PAGE: &str = "Track";
const MIX_PAGE: &str = "Mix";
const OTHER_PAGE: &str = "Other";
const BASE_BIND: &str = "@base";
/// The widest a horizontal page runs before it is wrapped: six knobs, twelve cells, as a hardware strip.
const AUTO_PER_ROW: usize = 6;

/// A parameter's range as the engine reports it; every part may be missing.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutoMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub def: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Array<{ label: string, value: number }>"))]
    pub states: Option<Vec<Value>>,
}

/// One parameter to place: its name, and its range if anything knows it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutoParam {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub meta: Option<AutoMeta>,
}

/// One family of the table.
struct Family {
    page: &'static str,
    drop: bool,
    rx: Rx,
}

fn family(page: &'static str, drop: bool, source: &str) -> Family {
    Family {
        page,
        drop,
        rx: Rx::new(source, false),
    }
}

/// The families IN ORDER, which is the algorithm; the `@` namespace first, in two entries whose order is the whole rule.
static FAMILIES: LazyLock<Vec<Family>> = LazyLock::new(|| {
    vec![
        family(GENERAL_PAGE, false, "^@base$"),
        family(TRACK_PAGE, true, "^@"),
        family(
            "Preset",
            true,
            r"\bpreset|browse|navigate|keylab|hardware|controller|\bundo\b|\bredo\b|oct (minus|plus)|(active|multi|selected|clear) filter|filter (preset|category)",
        ),
        family(
            "MIDI",
            true,
            r"\bmpe\b|\bmidi\b|pitch ?bend|program ?change|aftertouch|\bctrl|mod ?wheel|\bpedal\b|channel pressure|\bnote (on|off)",
        ),
        family("Reverb", false, r"reverb|\breverb|\bhall\b|\bplate\b|pre.?delay"),
        family("Delay", false, r"\bdelay|\becho\b|feedback|\btape\b"),
        family(
            "Modulation",
            false,
            r"chorus|flanger|phaser|ensemble|tremolo|vibrato|\blfo\b",
        ),
        family(
            "Drive",
            false,
            r"disto|drive|saturat|overdrive|\bfuzz\b|harmonic|bit ?crush|waveshap",
        ),
        family(
            "Dynamics",
            false,
            r"\bcomp\b|compress|limit|\bgate\b|expander|threshold|\bratio\b|make ?up|sidechain|\bknee\b",
        ),
        family(
            "EQ",
            false,
            r"\beq\b|equali|shelf|\bbell\b|presence|treble|\btone\b|bright|\bband\b",
        ),
        family("Filter", false, r"filter|cutoff|resonance|formant|\bhpf?\b|\blpf?\b"),
        family(
            "Envelope",
            false,
            r"\benv\b|envelope|\badsr\b|attack|\bdecay\b|sustain|release|\bhold\b",
        ),
        family(
            "Oscillator",
            false,
            r"\bosc\b|oscillator|\bwave\b|pulse|\bsaw\b|square|\bsub\b|\bnoise\b",
        ),
        family(
            "Tuning",
            false,
            r"tune|tuning|detune|transpose|\bpitch\b|octave|semitone|\bcent\b|glide|portamento|\bscale\b|unison",
        ),
        family(
            MIX_PAGE,
            false,
            r"level|volume|\bgain\b|\bmix\b|\bdry\b|\bwet\b|\bpan\b|width|output|master|input|balance",
        ),
    ]
});

static BLEND_WORD: LazyLock<Rx> = LazyLock::new(|| Rx::new(r"\b(dry|wet|mix)\b", false));

fn is_stage(name: &str) -> bool {
    name == INPUT_VOLUME_PARAM || name == MIX_PARAM || name == OUTPUT_VOLUME_PARAM
}

/// `MPE_PitchBend_3` and `Pitch Bend` are the same string to a pattern.
fn readable(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c == '_' || c == '-' {
            if !gap {
                out.push(' ');
            }
            gap = true;
        } else {
            gap = false;
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Which family a parameter's NAME suggests. The engine's stage is the module's mix corner, never a mixer-strip control.
pub fn family_of(name: &str) -> &'static str {
    if is_stage(name) {
        return MIX_PAGE;
    }
    let s = readable(name);
    FAMILIES.iter().find(|f| f.rx.test(&s)).map_or(OTHER_PAGE, |f| f.page)
}

fn dropped(page: &str) -> bool {
    FAMILIES.iter().any(|f| f.drop && f.page == page)
}

/// Whether a parameter is plumbing the proposal skips.
pub fn is_plumbing(name: &str) -> bool {
    dropped(family_of(name))
}

/// Whether a parameter is the module's own blend: the test is the family, not the word.
pub fn is_blend(name: &str) -> bool {
    is_stage(name) || (family_of(name) == MIX_PAGE && BLEND_WORD.test(&readable(name)))
}

/// One page to lay out: its name, and its parameters in order.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Page {
    pub page: String,
    pub params: Vec<AutoParam>,
}

/// The pages that got something, never a `drop` one, in the table's own order.
pub fn group_params(params: &[AutoParam]) -> Vec<Page> {
    let mut by_page: Vec<(&'static str, Vec<AutoParam>)> = Vec::new();
    for p in params {
        let page = family_of(&p.name);
        if dropped(page) {
            continue;
        }
        match by_page.iter_mut().find(|(n, _)| *n == page) {
            Some((_, held)) => held.push(p.clone()),
            None => by_page.push((page, vec![p.clone()])),
        }
    }
    let order = FAMILIES
        .iter()
        .filter(|f| !f.drop)
        .map(|f| f.page)
        .chain(std::iter::once(OTHER_PAGE));
    let mut out = Vec::new();
    for page in order {
        if let Some((_, held)) = by_page.iter().find(|(n, _)| *n == page) {
            out.push(Page {
                page: page.to_owned(),
                params: held.clone(),
            });
        }
    }
    out
}

struct Shape {
    cols: usize,
    rows: usize,
    pages: Vec<(String, Value)>,
}

/// One shape of the panel: every page placed `per_row` to a row, numbered across the whole panel.
fn shape(pages: &[Page], per_row: usize) -> Shape {
    let (mut n, mut width, mut height) = (0, 1, 1);
    let mut out = Vec::new();
    for g in pages {
        let mut widgets = Map::new();
        for (i, p) in g.params.iter().enumerate() {
            let (sc, sr) = (i % per_row, i / per_row);
            let states = p
                .meta
                .as_ref()
                .and_then(|m| m.states.as_ref())
                .filter(|s| !s.is_empty());
            width = width.max((sc + 1) * KNOB_SPAN);
            height = height.max((sr + 1) * KNOB_SPAN);
            n += 1;
            let mut w = Map::new();
            w.insert("type".into(), json!(if states.is_some() { "dropdown" } else { "knob" }));
            w.insert("col".into(), json!(sc * KNOB_SPAN));
            w.insert("row".into(), json!(sr * KNOB_SPAN));
            w.insert("bind".into(), json!(p.name));
            w.insert("label".into(), json!(p.name.strip_prefix('@').unwrap_or(&p.name)));
            w.insert("colSpan".into(), json!(KNOB_SPAN));
            if states.is_none() {
                let m = p.meta.clone().unwrap_or_default();
                w.insert("rowSpan".into(), json!(KNOB_SPAN));
                w.insert("min".into(), json!(m.min.unwrap_or(0.0)));
                w.insert("max".into(), json!(m.max.unwrap_or(1.0)));
            } else {
                let all = p.meta.as_ref().and_then(|m| m.states.clone()).unwrap_or_default();
                w.insert("options".into(), Value::Array(all));
            }
            widgets.insert(format!("w{n}"), Value::Object(w));
        }
        out.push((g.page.clone(), Value::Object(widgets)));
    }
    Shape {
        cols: width,
        rows: height,
        pages: out,
    }
}

fn panel(s: &Shape, single: bool) -> Value {
    if single {
        let widgets = s.pages.first().map_or_else(|| json!({}), |p| p.1.clone());
        return json!({ "cols": s.cols, "rows": s.rows, "widgets": widgets });
    }
    let tabs: Vec<Value> = s
        .pages
        .iter()
        .map(|(name, w)| json!({ "name": name, "widgets": w }))
        .collect();
    json!({ "cols": s.cols, "rows": s.rows, "widgets": {}, "tabs": tabs })
}

/// A whole panel proposed from a parameter list, in both its shapes.
pub fn auto_organize(params: &[AutoParam]) -> Value {
    // What the engine's stage offers REPLACES the plugin's own: its dry/wet under `@mix`, its levels under the volumes.
    let has = |n: &str| params.iter().any(|p| p.name == n);
    let held = |n: &str| {
        (has(MIX_PARAM) && own_mix_side(n).is_some())
            || (has(INPUT_VOLUME_PARAM) && own_level_side(n) == Some(LevelSide::Input))
            || (has(OUTPUT_VOLUME_PARAM) && own_level_side(n) == Some(LevelSide::Output))
    };
    // …and the stage itself is the card's footer, never a control on the panel.
    let kept: Vec<AutoParam> = params
        .iter()
        .filter(|p| !held(&p.name) && !is_stage(&p.name))
        .cloned()
        .collect();
    let groups = group_params(&kept);
    if groups.is_empty() {
        return json!({ "cols": KNOB_SPAN, "rows": 1, "widgets": {} });
    }
    // `@base` leads its page and a blend closes it, so both shapes read the same way round.
    let pages: Vec<Page> = groups
        .into_iter()
        .map(|g| {
            let pick = |f: &dyn Fn(&AutoParam) -> bool| g.params.iter().filter(|p| f(p)).cloned().collect::<Vec<_>>();
            let mut params = pick(&|p| p.name == BASE_BIND);
            params.extend(pick(&|p| p.name != BASE_BIND && !is_blend(&p.name)));
            params.extend(pick(&|p| p.name != BASE_BIND && is_blend(&p.name)));
            Page { page: g.page, params }
        })
        .collect();
    let slot_rows = LAYOUT_MAX / KNOB_SPAN;
    let longest = pages.iter().map(|g| g.params.len()).max().unwrap_or(0);
    let per_row = longest.div_ceil(slot_rows).clamp(1, AUTO_PER_ROW);
    let per = per_row * slot_rows;
    let mut wrapped = Vec::new();
    for g in pages {
        if g.params.len() <= per {
            wrapped.push(g);
            continue;
        }
        for (part, chunk) in g.params.chunks(per).enumerate() {
            let page = if part == 0 {
                g.page.clone()
            } else {
                format!("{} {}", g.page, part + 1)
            };
            wrapped.push(Page {
                page,
                params: chunk.to_vec(),
            });
        }
    }
    let across = shape(&wrapped, per_row);
    let down = shape(&wrapped, LAYOUT_MAX / KNOB_SPAN);
    // One family is a panel, not a tab: the second is what makes the list.
    let single = across.pages.len() <= 1;
    let mut horizontal = panel(&across, single);
    // The vertical twin only where the panel is too wide to serve both shapes.
    if across.cols > LAYOUT_MAX {
        horizontal["vertical"] = panel(&down, single);
    }
    horizontal
}

#[cfg(test)]
#[path = "auto_tests.rs"]
mod tests;
