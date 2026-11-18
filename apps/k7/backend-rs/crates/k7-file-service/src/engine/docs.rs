//! What is sent (2231, 2232): a client's document kept as JSON, and the three things done to any document on its way out
//! (the audition, the take's hole, the group fold). A document the song crate's type refuses goes with the fold alone.
//! What a reader gets wrong: the hole and the audition apply only to a document the typed reader accepts; the rest is counted.

use serde_json::Value;
use song::doc::{ProjectDoc, with_group_mix};

/// Which event a take is recorded into and the punch it replaces: `to` absent runs to the stop.
#[derive(Debug, Clone, PartialEq)]
pub struct TakeArm {
    pub track: String,
    pub event_beats: f64,
    pub from: f64,
    pub to: Option<f64>,
}

/// A document ready to send, and whether the typed reader took it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sent {
    pub doc: Value,
    pub typed: bool,
}

/// What the engine is sent for `doc`: the audition replaces the song, the hole is cut, then the groups fold onto their members.
pub fn prepare(doc: &Value, arm: Option<&TakeArm>, audition: Option<&str>) -> Sent {
    match serde_json::from_value::<ProjectDoc>(doc.clone()) {
        Ok(mut d) => {
            if let Some(a) = arm {
                song::clips::with_take_hole(
                    &mut d,
                    &a.track,
                    a.from,
                    a.to.unwrap_or(f64::INFINITY),
                );
            }
            if let Some(id) = audition {
                d = song::clips::audition_timeline(d, id);
            }
            with_group_mix(&mut d);
            match serde_json::to_value(&d) {
                Ok(doc) => Sent { doc, typed: true },
                Err(_) => Sent {
                    doc: fold_json(doc),
                    typed: false,
                },
            }
        }
        Err(_) => Sent {
            doc: fold_json(doc),
            typed: false,
        },
    }
}

/// `withGroupMix` on JSON alone: each group's mute and solo ORed onto its members.
pub fn fold_json(doc: &Value) -> Value {
    let mut out = doc.clone();
    let groups: Vec<(String, bool, bool)> = doc
        .get("groups")
        .and_then(Value::as_array)
        .map(|gs| {
            gs.iter()
                .filter_map(|g| {
                    let id = g.get("id")?.as_str()?.to_string();
                    let on = |k: &str| g.get(k).and_then(Value::as_bool) == Some(true);
                    Some((id, on("mute"), on("solo")))
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(tracks) = out.get_mut("tracks").and_then(Value::as_array_mut) else {
        return out;
    };
    for t in tracks {
        let Some(group) = t.get("group").and_then(Value::as_str) else {
            continue;
        };
        // A group listed twice is read as `new Map(pairs)` reads it: the last wins.
        let Some((_, mute, solo)) = groups.iter().rev().find(|(id, _, _)| id == group) else {
            continue;
        };
        if !(*mute || *solo) {
            continue;
        }
        let was = |k: &str| t.get(k).and_then(Value::as_bool) == Some(true);
        let (m, s) = (was("mute") || *mute, was("solo") || *solo);
        if let Some(o) = t.as_object_mut() {
            o.insert("mute".into(), Value::Bool(m));
            o.insert("solo".into(), Value::Bool(s));
        }
    }
    out
}
