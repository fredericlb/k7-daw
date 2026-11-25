// curves/file.rs — a curves/<id>.k7x file decoded strict, with parseCurve's defaults; after CurveSchema and parseCurve.
// A reader would get wrong that nothing here clamps or falls back to a default for a bad word: the schema refuses it (202).

use crate::files::scalar::{NonEmpty, NonNeg, Pos, Timecode, Unit};
use crate::files::{Check, DeError, Finding, decode_with, from_map};
use k7x::{MapNode, Node, Scalar};
use serde::de::{self, Deserialize, Deserializer, IgnoredAny};
use serde::{Deserialize as Derive, Serialize};
use song_schema::{AdsrSpec, CurveSpec, Cut, LfoSpec, PointSpec, PointsSpec, Polarity, Shape, TimeSig, parse_duration};

/// A curve file as `parseCurve` answers it: the pilot's spec, and the conditions when there are any.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CurveFile {
    #[serde(flatten)]
    pub spec: CurveSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<CurveCondition>>,
}

impl Check for CurveFile {}

/// WHEN a curve runs; timecodes as written, positions in the song.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CurveCondition {
    Start {
        at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    End {
        at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    Between {
        start: String,
        end: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    Event {
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
}

/// A list that is not empty.
struct Some1<T>(Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Some1<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Vec::<T>::deserialize(d)?;
        if v.is_empty() {
            return Err(de::Error::custom("expected at least 1 point"));
        }
        Ok(Some1(v))
    }
}

/// An on/off point's value: 0 or 1 and nothing between.
#[derive(Clone, Copy)]
struct OnOffValue(f64);

impl<'de> Deserialize<'de> for OnOffValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(d)?;
        if v == 0.0 || v == 1.0 {
            Ok(OnOffValue(v))
        } else {
            Err(de::Error::custom("an on/off curve is 0 or 1"))
        }
    }
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct LfoIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    #[serde(rename = "conditions")]
    _conditions: Option<IgnoredAny>,
    polarity: Option<Polarity>,
    amplitude: Option<Unit>,
    cut: Option<Cut>,
    shape: Option<Shape>,
    frequency: Option<Pos>,
    phase: Option<Unit>,
    duration: Option<Timecode>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct CustomIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    #[serde(rename = "conditions")]
    _conditions: Option<IgnoredAny>,
    polarity: Option<Polarity>,
    amplitude: Option<Unit>,
    cut: Option<Cut>,
    points: Some1<CustomPoint>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct CustomPoint {
    time: Timecode,
    value: f64,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct OnOffIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    #[serde(rename = "conditions")]
    _conditions: Option<IgnoredAny>,
    points: Some1<OnOffPoint>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct OnOffPoint {
    time: Timecode,
    value: OnOffValue,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct AdsrIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    #[serde(rename = "conditions")]
    _conditions: Option<IgnoredAny>,
    polarity: Option<Polarity>,
    amplitude: Option<Unit>,
    cut: Option<Cut>,
    attack: Option<NonNeg>,
    decay: Option<NonNeg>,
    sustain: Option<NonNeg>,
    release: Option<NonNeg>,
    level: Option<Unit>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct StartIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    at: Timecode,
    retrigger: Option<bool>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct BetweenIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    start: Timecode,
    end: Timecode,
    retrigger: Option<bool>,
}

#[derive(Derive)]
#[serde(deny_unknown_fields)]
struct EventIn {
    #[serde(rename = "type")]
    _type: IgnoredAny,
    event: NonEmpty,
    retrigger: Option<bool>,
}

const CURVE_KINDS: &[&str] = &["lfo", "custom", "adsr", "onoff"];
const CONDITION_KINDS: &[&str] = &["start", "end", "between", "event"];

/// Where an `adsr` level lands when the file does not say.
const ADSR_DEFAULT_LEVEL: f64 = 0.5;

fn err(message: &str, at: usize) -> DeError {
    DeError {
        message: message.to_owned(),
        at: Some(at),
    }
}

/// The word a map's `type` key holds, and it is one of `kinds`.
fn kind_of<'a>(map: &MapNode<'a>, kinds: &'static [&'static str]) -> Result<&'a str, DeError> {
    let Some(e) = map.entries.iter().rev().find(|e| e.key == "type") else {
        return Err(err("missing key `type`", map.start));
    };
    let Node::Scalar {
        value: Scalar::Str(s), ..
    } = e.value
    else {
        return Err(err("expected the name of a kind", e.value.start()));
    };
    if kinds.contains(&s) {
        Ok(s)
    } else {
        let mut unknown = <DeError as de::Error>::unknown_variant(s, kinds);
        unknown.at = Some(e.value.start());
        Err(unknown)
    }
}

fn condition(node: &Node) -> Result<CurveCondition, DeError> {
    let Node::Map(m) = node else {
        return Err(err("expected an object", node.start()));
    };
    Ok(match kind_of(m, CONDITION_KINDS)? {
        "start" => {
            let c = from_map::<StartIn>(m)?;
            CurveCondition::Start {
                at: c.at.0,
                retrigger: c.retrigger,
            }
        }
        "end" => {
            let c = from_map::<StartIn>(m)?;
            CurveCondition::End {
                at: c.at.0,
                retrigger: c.retrigger,
            }
        }
        "between" => {
            let c = from_map::<BetweenIn>(m)?;
            CurveCondition::Between {
                start: c.start.0,
                end: c.end.0,
                retrigger: c.retrigger,
            }
        }
        _ => {
            let c = from_map::<EventIn>(m)?;
            CurveCondition::Event {
                event: c.event.0,
                retrigger: c.retrigger,
            }
        }
    })
}

/// The `conditions` list; none written, or an empty list, is absent.
fn conditions_of(root: &MapNode) -> Result<Option<Vec<CurveCondition>>, DeError> {
    let Some(e) = root.entries.iter().rev().find(|e| e.key == "conditions") else {
        return Ok(None);
    };
    let Node::List { items, .. } = e.value else {
        return Err(err("expected a list", e.value.start()));
    };
    let list = items.iter().map(condition).collect::<Result<Vec<_>, _>>()?;
    Ok((!list.is_empty()).then_some(list))
}

/// The cut a file carries: `none` is the default and is not kept.
fn kept(cut: Option<Cut>) -> Option<Cut> {
    cut.filter(|c| *c != Cut::None)
}

fn lfo(c: LfoIn) -> Result<CurveSpec, DeError> {
    // A length of no beats is for ever and is not kept; it is read in 4/4, before the song's signature is known.
    let duration = match c.duration {
        Some(tc) => {
            let sig = TimeSig::new(4, 4).map_err(|e| <DeError as de::Error>::custom(e.message()))?;
            let beats = parse_duration(&tc.0, sig).map_err(|e| <DeError as de::Error>::custom(e.message()))?;
            (beats > 0.0).then_some(tc.0)
        }
        None => None,
    };
    Ok(CurveSpec::Lfo(LfoSpec {
        polarity: c.polarity.unwrap_or(Polarity::Unipolar),
        amplitude: c.amplitude.map_or(1.0, |a| a.0),
        cut: kept(c.cut),
        shape: c.shape.unwrap_or(Shape::Sine),
        frequency: c.frequency.map_or(1.0, |f| f.0),
        phase: c.phase.map_or(0.0, |p| p.0),
        duration,
    }))
}

fn custom(c: CustomIn) -> CurveSpec {
    CurveSpec::Custom(PointsSpec {
        polarity: c.polarity.unwrap_or(Polarity::Unipolar),
        amplitude: c.amplitude.map_or(1.0, |a| a.0),
        cut: kept(c.cut),
        points: c
            .points
            .0
            .into_iter()
            .map(|p| PointSpec {
                time: p.time.0,
                value: p.value.clamp(0.0, 1.0),
            })
            .collect(),
    })
}

fn onoff(c: OnOffIn) -> CurveSpec {
    CurveSpec::Onoff(PointsSpec {
        polarity: Polarity::Unipolar,
        amplitude: 1.0,
        cut: None,
        points: c
            .points
            .0
            .into_iter()
            .map(|p| PointSpec {
                time: p.time.0,
                value: p.value.0,
            })
            .collect(),
    })
}

fn adsr(c: AdsrIn) -> CurveSpec {
    let ms = |v: Option<NonNeg>| v.map_or(0.0, |n| n.0);
    CurveSpec::Adsr(AdsrSpec {
        polarity: c.polarity.unwrap_or(Polarity::Unipolar),
        amplitude: c.amplitude.map_or(1.0, |a| a.0),
        cut: kept(c.cut),
        attack: ms(c.attack),
        decay: ms(c.decay),
        sustain: ms(c.sustain),
        release: ms(c.release),
        level: c.level.map_or(ADSR_DEFAULT_LEVEL, |l| l.0),
    })
}

fn read_curve(root: &MapNode) -> Result<CurveFile, DeError> {
    let kind = kind_of(root, CURVE_KINDS)?;
    let conditions = conditions_of(root)?;
    let spec = match kind {
        "lfo" => lfo(from_map::<LfoIn>(root)?)?,
        "custom" => custom(from_map::<CustomIn>(root)?),
        "adsr" => adsr(from_map::<AdsrIn>(root)?),
        _ => onoff(from_map::<OnOffIn>(root)?),
    };
    Ok(CurveFile { spec, conditions })
}

/// The longest quoted word, and the longest message, a refusal here carries (decision 224).
const WORD_MAX: usize = 40;
const MESSAGE_MAX: usize = 300;

/// A message with every long quoted word cut short, so a word of 5,000 characters is not a message of 5,000.
fn clipped(message: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<(char, usize)> = None;
    for ch in message.chars() {
        match quote {
            Some((q, _)) if ch == q => {
                out.push(ch);
                quote = None;
            }
            Some((q, n)) if n >= WORD_MAX => quote = Some((q, n + 1)),
            Some((q, n)) => {
                if n + 1 == WORD_MAX {
                    out.push_str("...");
                } else {
                    out.push(ch);
                }
                quote = Some((q, n + 1));
            }
            None => {
                out.push(ch);
                if ch == '`' || ch == '"' {
                    quote = Some((ch, 0));
                }
            }
        }
    }
    out.chars().take(MESSAGE_MAX).collect()
}

pub fn decode_curve(file: &str, text: &str) -> Result<CurveFile, Finding> {
    decode_with(file, text, read_curve).map_err(|f| Finding {
        message: clipped(&f.message),
        ..f
    })
}
