//! answer.rs — `/waveform`'s answer from a sidecar's envelope on: the shade, `detected`, `onsetEdits`, the keys in the order the
//! frontend's spec pins, and the memo signature (2420, 2424). What a reader gets wrong: a non-finite number is `null` on the wire, as `JSON.stringify` writes it.

use serde_json::{Map, Value, json};
use song::clips::{ClipDoc, js_number};

use super::onsets::{Edits, resolve_onsets};
use super::region::{Envelope, GridLine, Region, TooLong, beat_grid, jmax, jmin, region_waveform};

/// The span of the file a placement shows, in file seconds; a `to` of 0 is the file's end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub from: f64,
    pub to: f64,
}

/// `materialWindow`: the clip's own window, or none when `sourceFrom` and `sourceTo` are both zero or absent.
pub fn material_window(c: &ClipDoc) -> Option<Window> {
    let truthy = |v: Option<f64>| v.is_some_and(|x| x != 0.0 && !x.is_nan());
    (truthy(c.source_from) || truthy(c.source_to)).then(|| Window {
        from: c.source_from.unwrap_or(0.0),
        to: c.source_to.unwrap_or(0.0),
    })
}

/// The wire's number: `JSON.stringify` writes a non-finite one as `null`.
fn n(x: f64) -> Value {
    json!(x)
}

fn list(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|&x| n(x)).collect())
}

fn line(l: &GridLine) -> Value {
    let mut m = Map::new();
    m.insert("t".into(), n(l.t));
    m.insert("bar".into(), Value::Bool(l.bar));
    if let Some(label) = &l.label {
        m.insert("label".into(), Value::String(label.clone()));
    }
    Value::Object(m)
}

/// `buildWaveform`: the region's columns, grid, warp marks and onsets, and the shade when the window cuts something.
pub fn build_waveform(r: &Region, window: Option<Window>) -> Result<Map<String, Value>, TooLong> {
    let mut m = Map::new();
    m.insert("rms".into(), list(&r.rms));
    m.insert("peak".into(), list(&r.peak));
    if let Some([low, mid, high]) = &r.bands {
        m.insert("low".into(), list(low));
        m.insert("mid".into(), list(mid));
        m.insert("high".into(), list(high));
    }
    m.insert("seconds".into(), n(r.seconds));
    let interval = if r.rms.is_empty() {
        0.0
    } else {
        (r.seconds / r.rms.len() as f64) * 1000.0
    };
    m.insert("intervalMs".into(), n(interval));
    m.insert("startBeat".into(), n(r.start_beat));
    m.insert("clipBeats".into(), n(r.clip_beats));
    m.insert(
        "grid".into(),
        Value::Array(beat_grid(r)?.iter().map(line).collect()),
    );
    let marks = r
        .warp_marks
        .iter()
        .map(|k| json!({ "orig": k.orig, "target": k.target, "src": k.src }));
    m.insert("warpMarks".into(), Value::Array(marks.collect()));
    let s = r.source_map;
    m.insert(
        "sourceMap".into(),
        json!({ "inPoint": s.in_point, "stretch": s.stretch, "zeroBeat": s.zero_beat }),
    );
    m.insert("transients".into(), list(&r.transients));
    if let Some(w) = window {
        let from = jmax(0.0, r.to_display(w.from));
        // An unwritten `to` is the file's end, which on this picture is its right edge.
        let to = if w.to > 0.0 {
            jmin(r.seconds, r.to_display(w.to))
        } else {
            r.seconds
        };
        if to > from && (from > 1e-6 || to < r.seconds - 1e-6) {
            m.insert("sourceWindow".into(), json!({ "from": from, "to": to }));
        }
    }
    Ok(m)
}

/// `JSON.stringify(edits)`: a list that is absent is left out; the memo signature carries the text.
pub fn edits_json(e: &Edits) -> String {
    let one = |k: &str, v: &Option<Vec<f64>>| {
        v.as_ref().map(|xs| {
            let items: Vec<String> = xs
                .iter()
                .map(|&x| {
                    if x.is_finite() {
                        js_number(x)
                    } else {
                        "null".into()
                    }
                })
                .collect();
            format!("\"{k}\":[{}]", items.join(","))
        })
    };
    let parts: Vec<String> = [one("added", &e.added), one("removed", &e.removed)]
        .into_iter()
        .flatten()
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn edits_value(e: &Edits) -> Value {
    let mut m = Map::new();
    if let Some(a) = &e.added {
        m.insert("added".into(), list(a));
    }
    if let Some(r) = &e.removed {
        m.insert("removed".into(), list(r));
    }
    Value::Object(m)
}

/// What a `/waveform` of a clip answers: the region over the corrected onsets, `detected` over the raw ones, `onsetEdits`.
pub fn waveform_answer(
    env: &Envelope<'_>,
    edits: &Edits,
    clip: &ClipDoc,
    (tempo, num, den): (f64, f64, f64),
    buckets: usize,
    window: Option<Window>,
) -> Result<Value, TooLong> {
    let mut drawn = clip.clone();
    if window.is_some() {
        drawn.source_from = None;
        drawn.source_to = None;
    }
    let corrected = resolve_onsets(env.transients, edits);
    let region = region_waveform(
        &Envelope {
            transients: &corrected,
            ..*env
        },
        &drawn,
        tempo,
        num,
        den,
        buckets,
    );
    let mut m = build_waveform(&region, window)?;
    m.insert(
        "detected".into(),
        list(&region.display_transients(env.transients)),
    );
    m.insert("onsetEdits".into(), edits_value(edits));
    Ok(Value::Object(m))
}

/// `wfSig`: the fingerprint and everything the region math reads, including the denominator and a `whole` draw's window (2424, D3).
pub fn wf_sig(
    fingerprint: &str,
    clip: &ClipDoc,
    (tempo, num, den): (f64, f64, f64),
    buckets: usize,
    window: Option<Window>,
) -> String {
    let opt = |v: Option<f64>| v.map_or(String::new(), js_number);
    let markers: Vec<String> = clip
        .warp
        .iter()
        .flat_map(|w| w.markers.iter())
        .flat_map(|&(b, s)| [js_number(b), js_number(s)])
        .collect();
    let fields = [
        fingerprint.to_string(),
        js_number(clip.offset),
        js_number(clip.length),
        js_number(clip.start),
        opt(clip.source_from),
        opt(clip.source_to),
        markers.join(","),
        js_number(tempo),
        js_number(num),
        js_number(den),
        buckets.to_string(),
        window.map_or(String::new(), |w| {
            format!("{}:{}", js_number(w.from), js_number(w.to))
        }),
    ];
    fields.join("|")
}

/// The memo's whole key: the signature, the sensitivity the onsets were found with, and the corrections.
pub fn answer_sig(sig: &str, sensitivity: f64, edits: &Edits) -> String {
    format!("{sig}|{}|{}", js_number(sensitivity), edits_json(edits))
}
