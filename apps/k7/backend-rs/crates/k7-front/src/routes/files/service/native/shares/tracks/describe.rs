//! describe.rs — the Markdown of the tracks export: one page per track with the chain it has in the project, and the README of the lot.
//! What a reader gets wrong: it describes the project's strip, which the audio files do not carry; they are rendered without it.

use std::fmt::Write;

use serde_json::Value;

/// The files a track got, as the archive names them.
pub struct Files {
    pub wav: Option<String>,
    pub mid: Option<String>,
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

/// A number as the document spells it.
fn number(v: &Value) -> String {
    v.as_f64().map_or_else(|| v.to_string(), |n| format!("{n}"))
}

/// The module files a node names, labelled.
const MODULE_FILES: [(&str, &str); 7] = [
    ("path", "preset"),
    ("irFile", "impulse response"),
    ("namFile", "amp model"),
    ("surgePatch", "Surge patch"),
    ("sfzFile", "SFZ"),
    ("deckFile", "deck"),
    ("faustDsp", "Faust"),
];

/// The curve range of an automation lane, as a sentence.
fn curve(lane: &Value) -> String {
    let points: Vec<(f64, f64)> = lane["points"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
        .collect();
    if points.is_empty() {
        return "no points".into();
    }
    let fold = |f: fn(f64, f64) -> f64, pick: fn(&(f64, f64)) -> f64| {
        points.iter().map(pick).reduce(f).unwrap_or(0.0)
    };
    let (lo, hi) = (fold(f64::min, |p| p.1), fold(f64::max, |p| p.1));
    let (a, b) = (fold(f64::min, |p| p.0), fold(f64::max, |p| p.0));
    format!(
        "{} points, from {lo} to {hi}, over beats {a} to {b}",
        points.len()
    )
}

/// The lanes of `automation` that drive `param` of node `id`.
fn lanes<'a>(automation: &'a [Value], id: &str, param: &str) -> impl Iterator<Item = &'a Value> {
    let (id, param) = (id.to_owned(), param.to_owned());
    automation
        .iter()
        .filter(move |l| text(l, "pluginId") == id && text(l, "param") == param)
}

/// Where a param's value comes from, as a suffix; nothing when the project's files for the node are not known.
fn origin_of(origin: &Value, param: &str) -> String {
    if !origin.is_object() {
        return String::new();
    }
    if origin["own"]
        .as_array()
        .is_some_and(|o| o.iter().any(|k| k == param))
    {
        " (set by this node)".into()
    } else if let Some(m) = origin["from"][param].as_str() {
        format!(" (inherited from `{m}`)")
    } else {
        " (module default)".into()
    }
}

/// The nodes of `chain` in order, each with its module and every param.
pub fn chain(
    out: &mut String,
    chain: &[Value],
    automation: &[Value],
    level: &str,
    origins: &Value,
) {
    if chain.is_empty() {
        out.push_str("No node.\n\n");
        return;
    }
    for (i, p) in chain.iter().enumerate() {
        let _ = writeln!(out, "{level} {}. {}\n", i + 1, text(p, "name"));
        let _ = writeln!(out, "- id: `{}`", text(p, "id"));
        let _ = writeln!(out, "- type: `{}`", text(p, "type"));
        let _ = writeln!(out, "- role: {}", text(p, "role"));
        let _ = writeln!(out, "- enabled: {}", p["enabled"] == true);
        for (key, label) in MODULE_FILES {
            let file = text(p, key);
            if !file.is_empty() {
                let _ = writeln!(out, "- {label}: `{file}`");
            }
        }
        let origin = &origins[text(p, "id").rsplit(':').next().unwrap_or_default()];
        if origin.is_object() {
            let chain: Vec<String> = origin["extends"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r.as_str().map(|r| format!("`{r}`")))
                .collect();
            let _ = writeln!(
                out,
                "- extends: {}",
                if chain.is_empty() {
                    "nothing".into()
                } else {
                    chain.join(" > ")
                }
            );
        }
        let mut params: Vec<(&String, &Value)> = p["params"]
            .as_object()
            .map(|o| o.iter().collect())
            .unwrap_or_default();
        params.sort_by(|a, b| a.0.cmp(b.0));
        if params.is_empty() {
            out.push_str("- params: none\n");
        } else {
            out.push_str("- params:\n");
        }
        for (name, value) in params {
            let _ = write!(out, "  - `{name}`: {}", number(value));
            let _ = write!(out, "{}", origin_of(origin, name));
            for lane in lanes(automation, text(p, "id"), name) {
                let _ = write!(out, " (automated: {})", curve(lane));
            }
            out.push('\n');
        }
        out.push('\n');
    }
}

/// The page of the track at `index` (0-based) in `doc`.
pub fn track_page(doc: &Value, index: usize, files: &Files, origins: &Value) -> String {
    let t = &doc["tracks"][index];
    let mut s = format!("# {}\n\n", text(t, "name"));
    let _ = writeln!(s, "- content: {}", text(t, "contentType"));
    let _ = writeln!(s, "- role: {}", text(t, "role"));
    let group = doc["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|g| text(g, "id") == text(t, "group"))
        .map_or("none", |g| text(g, "name"));
    let _ = writeln!(s, "- group: {group}");
    if t["generated"] == true {
        s.push_str("- audio and MIDI: none, this lane is generated by a pattern and has no fixed audio or MIDI\n");
    } else {
        let _ = writeln!(s, "- audio: {}", file_or_none(&files.wav));
        let _ = writeln!(s, "- MIDI: {}", file_or_none(&files.mid));
    }
    if t["frozenMaterial"]
        .as_array()
        .is_some_and(|m| !m.is_empty())
    {
        s.push_str(
            "- frozen: the audio and MIDI are the clips the author placed, not the lane's render\n",
        );
    }
    s.push_str("\n## Chain in the project\n\n");
    s.push_str("The audio file does not carry these nodes.\n\n");
    let none: Vec<Value> = vec![];
    let automation = t["automation"].as_array().unwrap_or(&none);
    chain(
        &mut s,
        t["plugins"].as_array().unwrap_or(&none),
        automation,
        "###",
        origins,
    );
    s.push_str("## Sends\n\n");
    let sends = t["sends"].as_array().unwrap_or(&none);
    if sends.is_empty() {
        s.push_str("No send.\n");
    }
    for send in sends {
        let to = doc["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| text(d, "id") == text(send, "to"))
            .map_or_else(|| text(send, "to"), |d| text(d, "name"));
        let _ = writeln!(
            s,
            "- to {to}: level {}{}",
            number(&send["level"]),
            if send["enabled"] == true {
                ""
            } else {
                " (off)"
            }
        );
    }
    let loose: Vec<&Value> = automation
        .iter()
        .filter(|l| {
            !t["plugins"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| text(p, "id") == text(l, "pluginId"))
        })
        .collect();
    if !loose.is_empty() {
        s.push_str("\n## Other automation\n\n");
        for l in loose {
            let _ = writeln!(s, "- `{}`: {}", text(l, "param"), curve(l));
        }
    }
    s
}

fn file_or_none(f: &Option<String>) -> String {
    f.as_ref()
        .map_or_else(|| "none".into(), |n| format!("`{n}`"))
}

/// The README at the root of the archive.
pub fn readme(doc: &Value, files: &[(String, Files)], origins: &Value) -> String {
    let mut s = format!("# {}\n\n", text(doc, "name"));
    let _ = writeln!(s, "- tempo: {} bpm", number(&doc["tempo"]));
    let _ = writeln!(s, "- sample rate: {} Hz", number(&doc["sampleRate"]));
    let _ = writeln!(
        s,
        "- time signature: {}/{}",
        number(&doc["timeSigNumerator"]),
        number(&doc["timeSigDenominator"])
    );
    let _ = writeln!(s, "- master volume: {} dB", number(&doc["masterVolume"]));
    s.push_str(
        "\nEach track has its audio rendered without any effect, a MIDI file of its notes and a page of the chain it has in the project.\n\n",
    );
    s.push_str("## Master chain\n\n");
    let none: Vec<Value> = vec![];
    chain(
        &mut s,
        doc["masterPlugins"].as_array().unwrap_or(&none),
        &[],
        "###",
        origins,
    );
    s.push_str("## Tracks\n\n");
    for (name, f) in files {
        let _ = writeln!(
            s,
            "- {name}: audio {}, MIDI {}",
            file_or_none(&f.wav),
            file_or_none(&f.mid)
        );
    }
    s
}
