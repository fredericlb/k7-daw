// graph/view/amp.rs — what an amp model FILE is: which runtime plays it and how many knobs it reacts to, read by the keys the engine reads.
// A reader would get wrong that a `.nam` is never opened (its format has no knob inputs) and a `.json` is read into a probe of five keys, never a Value (425).

use crate::graph::Spend;
use crate::source::{Kind, Source, SourceError, Span};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::collections::HashMap;
use std::fmt;

/// The knobs the engine's amp exposes: `Knob 1` to `Knob 3`, whatever a model declares beyond.
pub const MAX_AMP_KNOBS: u32 = 3;
/// A model file larger than this is not read.
pub const MAX_MODEL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmpKind {
    Nam,
    AidaX,
    Proteus,
}

impl AmpKind {
    pub fn name(self) -> &'static str {
        match self {
            AmpKind::Nam => "nam",
            AmpKind::AidaX => "aida-x",
            AmpKind::Proteus => "proteus",
        }
    }
}

/// One model as a panel sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmpModel {
    pub kind: AmpKind,
    pub knobs: u32,
}

/// An element or a value that is a number or something else.
#[derive(Clone, Copy, Debug, Default)]
enum Num {
    #[default]
    Other,
    Null,
    Is(f64),
}

struct NumVisitor;

impl<'de> Visitor<'de> for NumVisitor {
    type Value = Num;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("anything")
    }

    fn visit_f64<E>(self, v: f64) -> Result<Num, E> {
        Ok(Num::Is(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Num, E> {
        Ok(Num::Is(v as f64))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Num, E> {
        Ok(Num::Is(v as f64))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Num, E> {
        Ok(Num::Other)
    }

    fn visit_str<E>(self, _: &str) -> Result<Num, E> {
        Ok(Num::Other)
    }

    fn visit_unit<E>(self) -> Result<Num, E> {
        Ok(Num::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Num, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Num::Other)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Num, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Num::Other)
    }
}

impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Num, D::Error> {
        d.deserialize_any(NumVisitor)
    }
}

/// A key's value as the probe needs it: absent, a list (with its last element), an object (with its `input_size`), or other.
#[derive(Clone, Copy, Debug, Default)]
enum Shape {
    #[default]
    Absent,
    List(Option<Num>),
    Object(Option<Num>),
    Other,
}

struct ShapeVisitor;

impl<'de> Visitor<'de> for ShapeVisitor {
    type Value = Shape;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("anything")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_str<E>(self, _: &str) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_unit<E>(self) -> Result<Shape, E> {
        Ok(Shape::Other)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Shape, A::Error> {
        let mut last = None;
        while let Some(n) = seq.next_element::<Num>()? {
            last = Some(n);
        }
        Ok(Shape::List(last))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Shape, A::Error> {
        let mut size = None;
        while let Some(key) = map.next_key::<String>()? {
            if key == "input_size" {
                size = Some(map.next_value::<Num>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Shape::Object(size))
    }
}

impl<'de> Deserialize<'de> for Shape {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Shape, D::Error> {
        d.deserialize_any(ShapeVisitor)
    }
}

/// A key that may be there with any value: its presence is the answer.
#[derive(Clone, Copy, Debug, Default)]
struct Present(bool);

impl<'de> Deserialize<'de> for Present {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Present, D::Error> {
        IgnoredAny::deserialize(d)?;
        Ok(Present(true))
    }
}

/// The keys of a model file the engine reads (`input_size` is read inside `model_data`); every other key is ignored unread.
#[derive(Default)]
struct Probe {
    layers: Shape,
    in_shape: Shape,
    model_data: Shape,
    state_dict: Present,
}

struct ProbeVisitor;

impl<'de> Visitor<'de> for ProbeVisitor {
    type Value = Probe;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    // A duplicate key is the last one, as `JSON.parse` has it.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Probe, A::Error> {
        let mut p = Probe::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "layers" => p.layers = map.next_value()?,
                "in_shape" => p.in_shape = map.next_value()?,
                "model_data" => p.model_data = map.next_value()?,
                "state_dict" => p.state_dict = map.next_value()?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(p)
    }
}

impl<'de> Deserialize<'de> for Probe {
    // Only an object: a sequence would read positionally as the struct's fields.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Probe, D::Error> {
        d.deserialize_map(ProbeVisitor)
    }
}

/// The knob inputs past the sound, capped at what the plugin has; none for a value that is no finite number.
fn knobs_of(n: Num) -> Option<u32> {
    let Num::Is(v) = n else { return None };
    if !v.is_finite() {
        return None;
    }
    let past_sound = v.round() - 1.0;
    Some(past_sound.clamp(0.0, f64::from(MAX_AMP_KNOBS)) as u32)
}

/// A parsed `.json` model: AIDA-X by `layers` and `in_shape`, Proteus by `model_data` and `state_dict`, or none.
pub fn amp_of_json(bytes: &[u8]) -> Option<AmpModel> {
    let p: Probe = serde_json::from_slice(bytes).ok()?;
    if let (Shape::List(_), Shape::List(last)) = (p.layers, p.in_shape) {
        return knobs_of(last?).map(|knobs| AmpModel {
            kind: AmpKind::AidaX,
            knobs,
        });
    }
    let sized = match p.model_data {
        Shape::Object(size) => size,
        Shape::List(_) => None,
        _ => return None,
    };
    if !p.state_dict.0 {
        return None;
    }
    knobs_of(match sized {
        None | Some(Num::Null) => Num::Is(1.0),
        Some(n) => n,
    })
    .map(|knobs| AmpModel {
        kind: AmpKind::Proteus,
        knobs,
    })
}

/// The model at an ABSOLUTE path, none when it is absent or neither format; `.json` files are read once per call (425).
pub(super) fn amp_model_info(
    source: &dyn Source,
    path: &str,
    memo: &mut HashMap<String, Option<AmpModel>>,
    spend: &mut Spend,
) -> Result<Option<AmpModel>, SourceError> {
    let lower = path.to_lowercase();
    if lower.ends_with(".nam") {
        return Ok(Some(AmpModel {
            kind: AmpKind::Nam,
            knobs: 0,
        }));
    }
    if !lower.ends_with(".json") {
        return Ok(None);
    }
    if let Some(known) = memo.get(path) {
        return Ok(*known);
    }
    let said = read_json(source, path, spend)?;
    memo.insert(path.to_owned(), said);
    Ok(said)
}

fn read_json(source: &dyn Source, path: &str, spend: &mut Spend) -> Result<Option<AmpModel>, SourceError> {
    let st = match source.stat(path) {
        Ok(st) => st,
        Err(SourceError::NotFound | SourceError::Outside | SourceError::Io(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if st.kind != Kind::File || st.len > MAX_MODEL_BYTES {
        return Ok(None);
    }
    let units = usize::try_from(st.len / 64).unwrap_or(usize::MAX).saturating_add(1);
    if !spend.charge(units) {
        return Ok(None);
    }
    match source.read(path, Span::All) {
        Ok(bytes) if bytes.len() as u64 <= MAX_MODEL_BYTES => Ok(amp_of_json(&bytes)),
        Ok(_) | Err(SourceError::NotFound | SourceError::Outside | SourceError::Io(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
