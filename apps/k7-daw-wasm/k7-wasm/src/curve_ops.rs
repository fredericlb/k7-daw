// curve_ops.rs — the curve.* and timecode.* operations, their argument checks and the table of open curves.
// A reader would get wrong that a handle is a hint: eviction and a trap both make it unknown, and the caller reopens.

use crate::error::{ErrorCode, WasmError};
use serde::{Deserialize, Serialize};
use song_schema::{
    Curve, CurvePreview, CurveSpec, SchemaError, TimeSig, XY, beats_to_duration, beats_to_position, parse_duration,
    parse_position, playhead, preview,
};

/// Samples across a picture when the request does not say.
const DEFAULT_SAMPLES: u32 = 160;

/// The most samples a request may ask for.
const MAX_SAMPLES: u32 = 4096;

/// How many curves stay open at once.
const SLOTS: usize = 256;

/// A handle's low bits name its slot, the rest its generation.
const SLOT_BITS: u32 = 8;

/// A generation keeps to the bits a handle has room for.
const GENERATION_MASK: u32 = (1 << (32 - SLOT_BITS)) - 1;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewArgs {
    pub spec: CurveSpec,
    pub num: u32,
    pub den: u32,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub n: Option<u32>,
    /// A song beat to place the playhead at.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub playhead: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OpenArgs {
    pub spec: CurveSpec,
    pub num: u32,
    pub den: u32,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub n: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CloseArgs {
    pub handle: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParseArgs {
    pub tc: String,
    pub num: u32,
    pub den: u32,
    /// A 1-based position when true, a 0-based duration when false.
    pub position: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FormatArgs {
    pub beats: f64,
    pub num: u32,
    pub den: u32,
    pub position: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OpenAnswer {
    pub handle: u32,
    pub preview: CurvePreview,
}

/// What `k7_curve_playhead` answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Head {
    At(XY),
    /// An envelope, or a beat before the start.
    Nothing,
    Unknown,
}

struct Slot {
    generation: u32,
    used: u64,
    curve: Option<Curve>,
}

/// The open curves: a fixed number of slots, the least recently used one evicted when all are taken.
#[derive(Default)]
pub struct CurveTable {
    slots: Vec<Slot>,
    clock: u64,
}

fn time_sig(num: u32, den: u32) -> Result<TimeSig, WasmError> {
    TimeSig::new(num, den).map_err(|_| {
        WasmError::bad_request(format!(
            "time signature {num}/{den}: both parts must be integers of at least 1"
        ))
    })
}

fn samples(n: Option<u32>) -> Result<usize, WasmError> {
    let n = n.unwrap_or(DEFAULT_SAMPLES);
    if (1..=MAX_SAMPLES).contains(&n) {
        Ok(n as usize)
    } else {
        Err(WasmError::bad_request(format!(
            "n must be between 1 and {MAX_SAMPLES}, got {n}"
        )))
    }
}

impl CurveTable {
    pub fn new() -> CurveTable {
        CurveTable::default()
    }

    /// Stores a curve and answers its handle; a slot is free, else the least recently used is replaced.
    fn insert(&mut self, curve: Curve) -> u32 {
        self.clock += 1;
        let free = self.slots.iter().position(|s| s.curve.is_none());
        let index = match free {
            Some(i) => i,
            None if self.slots.len() < SLOTS => {
                self.slots.push(Slot {
                    generation: 0,
                    used: 0,
                    curve: None,
                });
                self.slots.len() - 1
            }
            None => self
                .slots
                .iter()
                .enumerate()
                .min_by_key(|(_, s)| s.used)
                .map_or(0, |(i, _)| i),
        };
        let clock = self.clock;
        match self.slots.get_mut(index) {
            Some(slot) => {
                slot.generation = (slot.generation + 1) & GENERATION_MASK;
                slot.used = clock;
                slot.curve = Some(curve);
                (slot.generation << SLOT_BITS) | index as u32
            }
            None => 0,
        }
    }

    /// The curve a handle names, marked as just used; none for a closed, evicted or never-issued one.
    fn touch(&mut self, handle: u32) -> Option<&Curve> {
        self.clock += 1;
        let clock = self.clock;
        let slot = self.slots.get_mut((handle & ((1 << SLOT_BITS) - 1)) as usize)?;
        if slot.generation != handle >> SLOT_BITS {
            return None;
        }
        slot.used = clock;
        slot.curve.as_ref()
    }

    pub fn open(&mut self, a: OpenArgs) -> Result<OpenAnswer, WasmError> {
        let (sig, n) = (time_sig(a.num, a.den)?, samples(a.n)?);
        let curve = Curve::resolve(&a.spec, sig)?;
        let picture = preview(&curve, n, None);
        Ok(OpenAnswer {
            handle: self.insert(curve),
            preview: picture,
        })
    }

    pub fn close(&mut self, a: CloseArgs) -> Result<(), WasmError> {
        let slot = self.slots.get_mut((a.handle & ((1 << SLOT_BITS) - 1)) as usize);
        match slot {
            Some(s) if s.generation == a.handle >> SLOT_BITS && s.curve.is_some() => {
                s.curve = None;
                Ok(())
            }
            _ => Err(WasmError::new(
                ErrorCode::UnknownHandle,
                format!("no open curve {}", a.handle),
            )),
        }
    }

    /// Where the playhead sits for `beat`; the one call made every frame.
    pub fn playhead(&mut self, handle: u32, beat: f64) -> Head {
        match self.touch(handle) {
            None => Head::Unknown,
            Some(curve) if beat.is_finite() => playhead(curve, beat).map_or(Head::Nothing, Head::At),
            Some(_) => Head::Nothing,
        }
    }
}

pub fn curve_preview(a: PreviewArgs) -> Result<CurvePreview, WasmError> {
    let (sig, n) = (time_sig(a.num, a.den)?, samples(a.n)?);
    let curve = Curve::resolve(&a.spec, sig)?;
    Ok(preview(&curve, n, a.playhead.filter(|b| b.is_finite())))
}

pub fn timecode_parse(a: ParseArgs) -> Result<f64, WasmError> {
    let sig = time_sig(a.num, a.den)?;
    let parsed = if a.position {
        parse_position(&a.tc, sig)
    } else {
        parse_duration(&a.tc, sig)
    };
    let beats = parsed?;
    if beats.is_finite() {
        Ok(beats)
    } else {
        Err(SchemaError::BadTimecode(format!("timecode '{}' is too large", a.tc)).into())
    }
}

pub fn timecode_format(a: FormatArgs) -> Result<String, WasmError> {
    let sig = time_sig(a.num, a.den)?;
    let text = if a.position {
        beats_to_position(a.beats, sig)
    } else {
        beats_to_duration(a.beats, sig)
    };
    Ok(text?)
}
