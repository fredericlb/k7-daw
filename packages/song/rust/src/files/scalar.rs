// files/scalar.rs — the checked scalars a file's fields are made of: a number in a range, a string of a shape, a window edge.
// A reader would get wrong where a check lives: in the type, so the error lands on the node being decoded (files/de.rs).

use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use song_schema::is_timecode;
use std::fmt;

macro_rules! checked_num {
    ($(#[$meta:meta])* $name:ident($ty:ty), $ok:expr, $msg:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub $ty);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = <$ty>::deserialize(d)?;
                let ok: fn(&$ty) -> bool = $ok;
                if ok(&v) { Ok(Self(v)) } else { Err(de::Error::custom($msg)) }
            }
        }
    };
}

macro_rules! checked_str {
    ($(#[$meta:meta])* $name:ident, $ok:expr, $msg:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = String::deserialize(d)?;
                let ok: fn(&str) -> bool = $ok;
                if ok(&v) { Ok(Self(v)) } else { Err(de::Error::custom($msg)) }
            }
        }
    };
}

pub(crate) use checked_num;
pub(crate) use checked_str;

checked_num!(NonNeg(f64), |v| *v >= 0.0, "expected a number of at least 0");
checked_num!(Pos(f64), |v| *v > 0.0, "expected a number above 0");
checked_num!(Unit(f64), |v| (0.0..=1.0).contains(v), "expected a number from 0 to 1");
checked_num!(PosInt(u64), |v| *v >= 1, "expected a whole number of at least 1");
checked_num!(AtLeast2(u64), |v| *v >= 2, "expected a whole number of at least 2");
checked_num!(MidiKey(u8), |v| *v <= 127, "expected a MIDI key from 0 to 127");
checked_num!(
    MasterDb(f64),
    |v| (-100.0..=12.0).contains(v),
    "expected a level from -100 to 12 dB"
);
checked_num!(
    MetronomeLevel(f64),
    |v| (0.2..=1.0).contains(v),
    "expected a level from 0.2 to 1"
);

checked_str!(NonEmpty, |s| !s.is_empty(), "expected a non-empty string");
checked_str!(
    Timecode,
    is_timecode,
    "expected a timecode like 25:1, 8:1:2 or 8:1:2:2.120"
);
checked_str!(
    SeqEdge,
    is_timecode,
    "expected a timecode like 5:1 — a position in the sequence"
);
checked_str!(LengthTimecode, is_timecode, "expected a duration like 8:0 — eight bars");
checked_str!(TimeSigText, is_time_sig, "expected a time signature like 4/4");
checked_str!(ConsoleRef, is_console_ref, "a console is @<library>/console/<system>");
checked_str!(LaneName, is_lane_name, "a lane name has no space and no '>'");

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_time_sig(s: &str) -> bool {
    s.split_once('/').is_some_and(|(n, d)| all_digits(n) && all_digits(d))
}

fn is_console_ref(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('@') else { return false };
    let mut parts = rest.split('/');
    let (lib, mid, system, more) = (parts.next(), parts.next(), parts.next(), parts.next());
    more.is_none()
        && mid == Some("console")
        && lib.is_some_and(|l| !l.is_empty())
        && system.is_some_and(|s| !s.is_empty())
}

/// JS's `\s`, which is not Rust's `is_whitespace` at U+0085 and U+FEFF.
pub(crate) fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

fn is_lane_name(s: &str) -> bool {
    !s.is_empty() && !s.contains('>') && !s.chars().any(js_space)
}

/// A clip's or a shot's window edge: seconds, or a musical position.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum WindowEdge {
    Seconds(f64),
    Timecode(String),
}

const EDGE_EXPECTED: &str = "expected seconds (0.25) or a timecode like 2:1";

impl<'de> Deserialize<'de> for WindowEdge {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(EdgeVisitor)
    }
}

struct EdgeVisitor;

impl Visitor<'_> for EdgeVisitor {
    type Value = WindowEdge;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(EDGE_EXPECTED)
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<WindowEdge, E> {
        if v >= 0.0 {
            Ok(WindowEdge::Seconds(v))
        } else {
            Err(E::custom("expected seconds of at least 0"))
        }
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<WindowEdge, E> {
        self.visit_f64(v as f64)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<WindowEdge, E> {
        self.visit_f64(v as f64)
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<WindowEdge, E> {
        if is_timecode(s) {
            Ok(WindowEdge::Timecode(s.to_owned()))
        } else {
            Err(E::custom(EDGE_EXPECTED))
        }
    }
}

impl WindowEdge {
    /// The edge when it is seconds.
    pub fn seconds(&self) -> Option<f64> {
        match self {
            WindowEdge::Seconds(s) => Some(*s),
            WindowEdge::Timecode(_) => None,
        }
    }
}

/// Whether `s` ends in `.<ext>` for one of `exts`, whatever the case.
pub fn has_ext(s: &str, exts: &[&str]) -> bool {
    s.rfind('.')
        .and_then(|i| s.get(i + 1..))
        .is_some_and(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

pub const VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "webm", "mkv"];
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "avif"];
pub const CAPTION_EXTS: &[&str] = &["srt", "vtt"];

checked_str!(
    PictureSource,
    |s| has_ext(s, VIDEO_EXTS) || has_ext(s, IMAGE_EXTS),
    "expected a video or an image file"
);
checked_str!(SoundFile, |s| !has_ext(s, VIDEO_EXTS), "a sound is an audio file");
checked_str!(
    CaptionSource,
    |s| has_ext(s, CAPTION_EXTS),
    "expected a subtitle file (.srt or .vtt)"
);
