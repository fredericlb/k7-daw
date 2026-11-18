// files/placement.rs — one event on a lane, as a timeline file holds it; after placementSchema.ts and keyframes.ts.
// A reader would get wrong which keys a shot takes: that is the compile's to check, since only it knows each ref's material.

use super::finding::keys;
use super::scalar::{NonEmpty, NonNeg, Timecode, WindowEdge};
use k7x::PathSeg;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A shot's transition in its map form; bare seconds are widened into it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FadeMap {
    pub over: NonNeg,
    #[serde(rename = "with", skip_serializing_if = "Option::is_none")]
    pub with_: Option<NonEmpty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<NonEmpty>,
}

/// A fade: a timecode duration on audio, seconds or a transition map on a shot.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Fade {
    Timecode(Timecode),
    Map(FadeMap),
}

impl<'de> Deserialize<'de> for Fade {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(FadeVisitor)
    }
}

struct FadeVisitor;

impl<'de> Visitor<'de> for FadeVisitor {
    type Value = Fade;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("seconds, a timecode or a transition map")
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Fade, E> {
        if v < 0.0 {
            return Err(E::custom("expected a number of at least 0"));
        }
        Ok(Fade::Map(FadeMap {
            over: NonNeg(v),
            with_: None,
            chain: None,
        }))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Fade, E> {
        self.visit_f64(v as f64)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Fade, E> {
        self.visit_f64(v as f64)
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Fade, E> {
        Timecode::deserialize(de::value::StrDeserializer::<E>::new(s)).map(Fade::Timecode)
    }

    fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Fade, A::Error> {
        FadeMap::deserialize(de::value::MapAccessDeserializer::new(map)).map(Fade::Map)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
    Hold,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyObject {
    pub t: f64,
    pub v: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
}

/// One key: a `[t, v]` pair, or a map where the ease has to be said.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AnimKey {
    Pair((f64, f64)),
    Object(KeyObject),
}

impl<'de> Deserialize<'de> for AnimKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(AnimKeyVisitor)
    }
}

struct AnimKeyVisitor;

impl<'de> Visitor<'de> for AnimKeyVisitor {
    type Value = AnimKey;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a [time, value] pair or a map with t and v")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<AnimKey, A::Error> {
        let t = seq
            .next_element::<f64>()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let v = seq
            .next_element::<f64>()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        if seq.next_element::<de::IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(3, &self));
        }
        Ok(AnimKey::Pair((t, v)))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<AnimKey, A::Error> {
        KeyObject::deserialize(de::value::MapAccessDeserializer::new(map)).map(AnimKey::Object)
    }
}

/// A property as written: a constant, or at least one key.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Animated {
    Const(f64),
    Keys(Vec<AnimKey>),
}

impl<'de> Deserialize<'de> for Animated {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(AnimatedVisitor)
    }
}

struct AnimatedVisitor;

impl<'de> Visitor<'de> for AnimatedVisitor {
    type Value = Animated;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number, or a list of keys")
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Animated, E> {
        Ok(Animated::Const(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Animated, E> {
        Ok(Animated::Const(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Animated, E> {
        Ok(Animated::Const(v as f64))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Animated, A::Error> {
        let mut out = Vec::new();
        while let Some(k) = seq.next_element::<AnimKey>()? {
            out.push(k);
        }
        if out.is_empty() {
            return Err(de::Error::custom("expected at least one key"));
        }
        Ok(Animated::Keys(out))
    }
}

/// How a shot moves inside its lane's frame; strict, so a mistyped property is named rather than ignored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<Animated>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<Animated>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Animated>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Animated>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<Animated>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Take {
    #[serde(rename = "ref")]
    pub ref_: NonEmpty,
    pub start: Timecode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompPiece {
    pub take: NonEmpty,
    pub from: Timecode,
    pub to: Timecode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crossfade {
    pub at: Timecode,
    pub length: Timecode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pitch {
    Tape,
    Keep,
}

/// One event: where, how many times, which part; a shot also says how long.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<NonEmpty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<WindowEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<WindowEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<WindowEdge>,
    #[serde(rename = "fadeIn", skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<Fade>,
    #[serde(rename = "fadeOut", skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<Fade>,
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_: Option<super::scalar::PosInt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain: Option<Gain>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub takes: Option<Vec<Take>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comp: Option<Vec<CompPiece>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossfades: Option<Vec<Crossfade>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<NonNeg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animate: Option<Animate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<NonEmpty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<Pitch>,
}

/// An event's own trim in dB, plus or minus 6.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Gain(pub f64);

impl<'de> Deserialize<'de> for Gain {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(d)?;
        if (-6.0..=6.0).contains(&v) {
            Ok(Gain(v))
        } else {
            Err(de::Error::custom("expected a trim from -6 to 6 dB"))
        }
    }
}

const ACTIVATION: &str =
    "an event with no `ref` is an ACTIVATION — it says WHEN a lane of shapes is on, and has no source for";

impl Placement {
    /// The first refusal the schema's refinement makes: the key it is about and the message.
    pub fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let (from, to) = (
            self.from.as_ref().and_then(WindowEdge::seconds),
            self.to.as_ref().and_then(WindowEdge::seconds),
        );
        if let (Some(from), Some(to)) = (from, to)
            && to <= from
        {
            return Some((keys(&["to"]), "'to' must be greater than 'from'".to_owned()));
        }
        let speed = self.speed.unwrap_or(1.0);
        let length = self.length.map_or(0.0, |l| l.0);
        let start = from.unwrap_or(0.0);
        if self.ref_.is_some() && speed < 0.0 && length > 0.0 && start + length * speed < -5e-4 {
            let runs = start / -speed;
            let message = format!(
                "backwards at {}× from {start}s, this shot runs out of source after {runs:.3}s — shorten it, or start it later in its source",
                -speed
            );
            return Some((keys(&["length"]), message));
        }
        if self.ref_.is_some() {
            return None;
        }
        let present = [
            ("from", self.from.is_some()),
            ("to", self.to.is_some()),
            ("fadeIn", self.fade_in.is_some()),
            ("fadeOut", self.fade_out.is_some()),
            ("speed", self.speed.is_some()),
            ("effect", self.effect.is_some()),
            ("silent", self.silent.is_some()),
            ("pitch", self.pitch.is_some()),
        ];
        present
            .iter()
            .find(|(_, here)| *here)
            .map(|(key, _)| (keys(&[key]), format!("{ACTIVATION} `{key}`")))
    }
}
