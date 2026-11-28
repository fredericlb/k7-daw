// master_controls.rs — what a `mastering` surface has knobs for: the controls in three groups, one per part of the quick-master node.
// The vocabulary a `surfaces/<id>.k7x` mapping is keyed by: the song crate accepts any key, and this is the list the page draws.
// A reader would get wrong that the labels are not here: they are the page's translated messages, keyed by `id` (docs/mastering.md).

use crate::bindings::SurfaceBinding;
use indexmap::IndexMap;
use serde::Serialize;

/// The three appliances inside one quick-master node, in the order the signal meets them.
pub const MASTER_PARTS: [&str; 3] = ["comp", "spread", "limiter"];

/// One knob of the strip: its mapping key, its part and the fallback its widget starts from.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct MasterControl {
    /// The mapping key, prefixed by its part, which is what groups the panel.
    pub id: &'static str,
    pub part: &'static str,
    /// The fallback range in the PARAM's own units: the module's own widget wins when it draws one.
    pub range: (f64, f64),
    /// Where the knob sits when nothing is known yet: never 0, which is a real value on most of these.
    pub home: f64,
    pub step: f64,
    /// Printed after the value: cosmetic.
    pub unit: &'static str,
    /// The note the scaffold prints beside the empty line.
    pub wants: &'static str,
    /// Not required by the gate: a surface written before it existed still draws.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// A CHOICE rather than a range: the param's values 0, 1, 2… named in order, drawn as a dropdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<&'static [&'static str]>,
}

const fn knob(
    id: &'static str,
    part: &'static str,
    range: (f64, f64),
    home: f64,
    step: f64,
    unit: &'static str,
    wants: &'static str,
) -> MasterControl {
    MasterControl {
        id,
        part,
        range,
        home,
        step,
        unit,
        wants,
        optional: false,
        choices: None,
    }
}

const fn optional(c: MasterControl) -> MasterControl {
    MasterControl { optional: true, ..c }
}

/// The controls in signal order, with NO normalisation anywhere on this surface, so a number can be checked against a meter.
pub const MASTER_CONTROLS: [MasterControl; 13] = [
    optional(knob(
        "comp.lowCut",
        "comp",
        (0.0, 300.0),
        0.0,
        1.0,
        "Hz",
        "the quick-master module — its low cut on the mid, before it compresses",
    )),
    knob(
        "comp.low",
        "comp",
        (0.0, 100.0),
        0.0,
        1.0,
        "%",
        "the SAME module — how much it compresses the lows",
    ),
    knob(
        "comp.mid",
        "comp",
        (0.0, 100.0),
        0.0,
        1.0,
        "%",
        "the SAME module — how much on the mids",
    ),
    knob(
        "comp.high",
        "comp",
        (0.0, 100.0),
        0.0,
        1.0,
        "%",
        "the SAME module — how much on the highs",
    ),
    knob(
        "comp.lowFreq",
        "comp",
        (40.0, 500.0),
        180.0,
        1.0,
        "Hz",
        "the SAME module — the crossover between lows and mids",
    ),
    knob(
        "comp.highFreq",
        "comp",
        (800.0, 8000.0),
        3000.0,
        10.0,
        "Hz",
        "the SAME module — the crossover between mids and highs",
    ),
    knob(
        "spread.width",
        "spread",
        (0.0, 200.0),
        100.0,
        1.0,
        "%",
        "the SAME module — the stereo width",
    ),
    knob(
        "spread.bassMono",
        "spread",
        (0.0, 300.0),
        0.0,
        1.0,
        "Hz",
        "the SAME module — the frequency below which the bass is brought to the centre",
    ),
    optional(knob(
        "limiter.hfLimit",
        "limiter",
        (0.0, 100.0),
        0.0,
        1.0,
        "%",
        "the SAME module — its HF limit, what it takes off the fastest highs",
    )),
    knob(
        "limiter.drive",
        "limiter",
        (0.0, 12.0),
        0.0,
        0.1,
        "dB",
        "the SAME module — the limiter's input gain",
    ),
    knob(
        "limiter.ceiling",
        "limiter",
        (-12.0, 0.0),
        0.0,
        0.1,
        "dB",
        "the SAME module — its ceiling",
    ),
    knob(
        "limiter.release",
        "limiter",
        (10.0, 500.0),
        120.0,
        1.0,
        "ms",
        "the SAME module — its release time",
    ),
    optional(MasterControl {
        choices: Some(&["Float", "24-bit", "16-bit"]),
        ..knob(
            "limiter.output",
            "limiter",
            (0.0, 2.0),
            0.0,
            1.0,
            "",
            "the SAME module — its output word length, float, 24-bit or 16-bit",
        )
    }),
];

/// The modules a surface's mapping names, deduplicated: more than one is legal, and worth SAYING rather than refusing.
pub fn mapped_modules(mapping: &IndexMap<String, Option<SurfaceBinding>>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in mapping.values().flatten() {
        if !b.module.is_empty() && !out.contains(&b.module) {
            out.push(b.module.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests;
