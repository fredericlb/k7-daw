// files/desk.rs — `mix.k7x` (the desk's cue sheet) and `views/mix.k7x` (which control drives which param), decoded for the editor's check.
// A reader would get wrong that a cue is a bare ref OR an object: a hand-written visitor reads it, since an untagged enum would only say "no variant".

use super::scalar::{checked_str, js_space};
use super::{Check, decode};
use crate::Finding;
use crate::views::nullable::Nullable;
use crate::views::surfaces::ViewBinding;
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

/// How many cue buttons a song has.
pub const MARKER_SLOTS: usize = 8;

const REF_EXPECTED: &str = "expected a workspace audio file path like records/hells-bells.flac";

/// A ref the cue sheet keys a record by: a workspace path already in its plain form.
fn is_record_ref(s: &str) -> bool {
    s.trim_matches(js_space) == s
        && !s.is_empty()
        && !s.contains('\\')
        && !s.starts_with(['@', '/'])
        && !s.ends_with('/')
        && s.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

checked_str!(RecordRef, is_record_ref, REF_EXPECTED);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CueFields {
    r#ref: RecordRef,
    markers: Option<Vec<Nullable<f64>>>,
}

/// One record's cues: bare, or with its cue points by button.
#[derive(Debug)]
pub struct Cue {
    pub reference: String,
    pub markers: Option<Vec<Nullable<f64>>>,
}

struct CueVisitor;

impl<'de> Visitor<'de> for CueVisitor {
    type Value = Cue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a record path or an object with `ref`")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Cue, E> {
        if is_record_ref(v) {
            Ok(Cue {
                reference: v.to_owned(),
                markers: None,
            })
        } else {
            Err(E::custom(REF_EXPECTED))
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<Cue, A::Error> {
        let f = CueFields::deserialize(MapAccessDeserializer::new(a))?;
        Ok(Cue {
            reference: f.r#ref.0,
            markers: f.markers,
        })
    }
}

impl<'de> Deserialize<'de> for Cue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(CueVisitor)
    }
}

/// `mix.k7x`: the cues, a sequence, duplicates being dropped on read rather than refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixFile {
    pub cues: Option<Vec<Cue>>,
}

fn key(name: &str) -> PathSeg {
    PathSeg::Key(name.to_owned())
}

impl Check for MixFile {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        for (i, cue) in self.cues.iter().flatten().enumerate() {
            let Some(markers) = &cue.markers else { continue };
            let at = |j: Option<usize>| {
                let mut p = vec![key("cues"), PathSeg::Index(i), key("markers")];
                p.extend(j.map(PathSeg::Index));
                p
            };
            if markers.len() > MARKER_SLOTS {
                return Some((at(None), format!("expected at most {MARKER_SLOTS} cue points")));
            }
            if let Some(j) = markers.iter().position(|m| m.0.is_some_and(|v| v < 0.0)) {
                return Some((at(Some(j)), "expected a number of at least 0".to_owned()));
            }
        }
        None
    }
}

pub fn decode_mix(file: &str, text: &str) -> Result<MixFile, Finding> {
    decode(file, text)
}

/// `views/mix.k7x`: control id to what it drives, `null` being a real value for a control not wired yet.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MixView(pub IndexMap<String, Nullable<ViewBinding>>);

impl Check for MixView {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        if self.0.contains_key("") {
            return Some((vec![key("")], "expected a non-empty key".to_owned()));
        }
        for (id, bound) in &self.0 {
            let Some(b) = &bound.0 else { continue };
            let targets = [("", &b.module, &b.param)]
                .into_iter()
                .chain(b.also.as_ref().map(|a| ("also", &a.module, &a.param)));
            for (head, module, param) in targets {
                for (field, text) in [("module", module), ("param", param)] {
                    if text.is_empty() {
                        let mut at = vec![key(id)];
                        if !head.is_empty() {
                            at.push(key(head));
                        }
                        at.push(key(field));
                        return Some((at, "expected a non-empty string".to_owned()));
                    }
                }
            }
        }
        None
    }
}

pub fn decode_mix_view(file: &str, text: &str) -> Result<MixView, Finding> {
    decode(file, text)
}
