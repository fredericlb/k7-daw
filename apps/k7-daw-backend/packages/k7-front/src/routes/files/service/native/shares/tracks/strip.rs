//! strip.rs — a project's engine document reduced to the clips of its audio tracks, which is what the stems render.
//! What a reader gets wrong: a track is renamed `t<index>` so two tracks of one name never share a stem file.

use serde_json::{Value, json};

/// The document to render, and the original index of each track kept in it.
pub struct Stripped {
    pub doc: Value,
    pub kept: Vec<usize>,
}

/// The stem name the render files track `index` under.
pub fn stem_name(index: usize) -> String {
    format!("t{index}")
}

/// A track that plays fixed audio: not a pattern's lane, and with at least one audio clip.
pub fn has_audio(track: &Value) -> bool {
    track["generated"] != json!(true) && audio_clips(track).next().is_some()
}

/// The clips the author placed: a frozen lane's `frozenMaterial`, else its `clips` (which then hold the render).
pub fn authored(track: &Value) -> impl Iterator<Item = &Value> {
    let frozen = track["frozenMaterial"].as_array().filter(|m| !m.is_empty());
    frozen
        .or_else(|| track["clips"].as_array())
        .into_iter()
        .flatten()
}

fn audio_clips(track: &Value) -> impl Iterator<Item = &Value> {
    authored(track).filter(|c| c["type"] == json!("audio") && c["disabled"] != json!(true))
}

/// `doc` with no effect anywhere, no level or routing, and only the audio clips of the tracks that have some.
pub fn strip(doc: &Value) -> Stripped {
    let mut out = doc.clone();
    let mut kept = Vec::new();
    let mut tracks = Vec::new();
    for (i, t) in doc["tracks"].as_array().into_iter().flatten().enumerate() {
        if !has_audio(t) {
            continue;
        }
        kept.push(i);
        tracks.push(bare(t, i));
    }
    out["tracks"] = Value::Array(tracks);
    out["masterPlugins"] = json!([]);
    out["masterVolume"] = json!(0);
    out["recordDir"] = json!("");
    if let Some(o) = out.as_object_mut() {
        o.remove("recordMute");
        o.remove("timelines");
    }
    for g in out["groups"].as_array_mut().into_iter().flatten() {
        g["mute"] = json!(false);
        g["solo"] = json!(false);
    }
    Stripped { doc: out, kept }
}

/// One track with its clips as placed and nothing else of its strip.
fn bare(track: &Value, index: usize) -> Value {
    let mut t = track.clone();
    t["name"] = json!(stem_name(index));
    t["clips"] = Value::Array(audio_clips(track).cloned().collect());
    if let Some(o) = t.as_object_mut() {
        o.remove("frozenMaterial");
    }
    t["plugins"] = json!([]);
    t["sends"] = json!([]);
    t["automation"] = json!([]);
    t["mute"] = json!(false);
    t["solo"] = json!(false);
    t["disabled"] = json!(false);
    t["inputChannels"] = json!([]);
    t["outputChannels"] = json!([]);
    t["monitor"] = json!("off");
    if let Some(o) = t.as_object_mut() {
        o.remove("highlight");
        o.remove("inputFrom");
    }
    t
}
