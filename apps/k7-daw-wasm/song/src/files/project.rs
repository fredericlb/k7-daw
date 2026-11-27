// files/project.rs — project.k7x and settings.k7x: the root's two tiers, its markers, the format level, the settings.
// A reader would get wrong that the level is read off the RAW text first: a newer file may use syntax this build cannot parse.

use super::Check;
use super::finding::{Finding, keys};
use super::scalar::{ConsoleRef, MasterDb, MetronomeLevel, NonEmpty, Pos, PosInt, TimeSigText, js_space};
use k7x::PathSeg;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use song_schema::{TimeSig, is_timecode, parse_position};
use std::fmt;

/// The project format this k7 reads and writes (song-schema's formatLevel.ts holds the history).
pub const FORMAT_LEVEL: u64 = 5;

/// The click's default level and the bars of count-in, which the TypeScript keeps in engine-protocol and project.ts.
/// The click's level when `settings.k7x` names none: Tracktion's own default.
pub const DEFAULT_METRONOME_LEVEL: f64 = 0.6;
/// The quietest click there is, so the schema, the command and the slider all stop in the same place.
pub const METRONOME_MIN: f64 = 0.2;
const DEFAULT_COUNT_IN: u32 = 1;

/// A marker's key: a musical timecode.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MarkerKey(String);

impl<'de> Deserialize<'de> for MarkerKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if is_timecode(&s) {
            Ok(MarkerKey(s))
        } else {
            Err(de::Error::custom("a marker's key is a musical timecode like 5:1:1"))
        }
    }
}

/// The `markers` map: a position, then its label, in the order written.
#[derive(Clone, Debug, PartialEq)]
pub struct Markers(pub Vec<(String, String)>);

impl<'de> Deserialize<'de> for Markers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(MarkersVisitor)
    }
}

struct MarkersVisitor;

impl<'de> Visitor<'de> for MarkersVisitor {
    type Value = Markers;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a map of timecodes to labels")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Markers, A::Error> {
        let mut out = Vec::new();
        while let Some((k, v)) = map.next_entry::<MarkerKey, String>()? {
            out.push((k.0, v));
        }
        Ok(Markers(out))
    }
}

impl Serialize for Markers {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

/// The picture's size: two numbers, or three.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Viewport(pub Vec<PosInt>);

impl<'de> Deserialize<'de> for Viewport {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Vec::<PosInt>::deserialize(d)?;
        if (2..=3).contains(&v.len()) {
            Ok(Viewport(v))
        } else {
            Err(de::Error::custom("expected 2 or 3 numbers"))
        }
    }
}

/// `layers` is gone: only the empty list is still accepted.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Layers(pub Vec<String>);

impl<'de> Deserialize<'de> for Layers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Vec::<String>::deserialize(d)?;
        if v.is_empty() {
            Ok(Layers(v))
        } else {
            Err(de::Error::custom(
                "`layers` is gone — the picture draws in the rows' order (`trackOrder`)",
            ))
        }
    }
}

/// The song's own settings, the `project: {…}` tier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo: Option<Pos>,
    #[serde(rename = "sampleRate", skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<PosInt>,
    #[serde(rename = "timeSignature", skip_serializing_if = "Option::is_none")]
    pub time_signature: Option<TimeSigText>,
    #[serde(rename = "masterVolume", skip_serializing_if = "Option::is_none")]
    pub master_volume: Option<MasterDb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console: Option<ConsoleRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules_from: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libraries_from: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<Layers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<NonEmpty>>,
}

/// project.k7x. `type`, `project_type` and a v1 wall-clock marker key are not accepted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<PosInt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectSettings>,
    #[serde(rename = "trackOrder", skip_serializing_if = "Option::is_none")]
    pub track_order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soloed: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlighted: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markers: Option<Markers>,
}

impl Check for ProjectFile {}

/// A newer format's refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooNew {
    pub level: u64,
    pub finding: Finding,
}

/// What project.k7x decoded to: the file, or the refusal of a newer format; never both.
#[derive(Clone, Debug, PartialEq)]
pub struct RootFile {
    pub project: Option<ProjectFile>,
    pub too_new: Option<TooNew>,
    /// Line and column of `project.timeSignature`, so the compile's finding about it can be placed.
    pub time_signature_at: Option<(u32, u32)>,
}

/// settings.k7x: a flat file whose unknown keys are kept.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metronome: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metronome_level: Option<MetronomeLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_in: Option<u64>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Check for Settings {}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(SettingsVisitor)
    }
}

struct SettingsVisitor;

impl<'de> Visitor<'de> for SettingsVisitor {
    type Value = Settings;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a settings file")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Settings, A::Error> {
        let mut s = Settings {
            metronome: None,
            metronome_level: None,
            count_in: None,
            extra: serde_json::Map::new(),
        };
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "metronome" => s.metronome = Some(map.next_value()?),
                "metronome_level" => s.metronome_level = Some(map.next_value()?),
                "count_in" => s.count_in = Some(map.next_value()?),
                _ => {
                    s.extra.insert(key, map.next_value()?);
                }
            }
        }
        Ok(s)
    }
}

/// The settings as a ProjectDoc carries them, each defaulted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SettingsDoc {
    pub metronome: bool,
    #[serde(rename = "metronomeLevel")]
    #[cfg_attr(feature = "ts", ts(rename = "metronomeLevel"))]
    pub metronome_level: f64,
    #[serde(rename = "countIn")]
    #[cfg_attr(feature = "ts", ts(rename = "countIn"))]
    pub count_in: u32,
}

impl Default for SettingsDoc {
    fn default() -> Self {
        SettingsDoc {
            metronome: false,
            metronome_level: DEFAULT_METRONOME_LEVEL,
            count_in: DEFAULT_COUNT_IN,
        }
    }
}

impl Settings {
    /// What `parseSettings` makes of the file: the level clamped to what the engine honours.
    pub fn doc(&self) -> SettingsDoc {
        SettingsDoc {
            metronome: self.metronome.unwrap_or(false),
            metronome_level: self
                .metronome_level
                .map_or(DEFAULT_METRONOME_LEVEL, |l| l.0.clamp(METRONOME_MIN, 1.0)),
            count_in: self
                .count_in
                .map_or(DEFAULT_COUNT_IN, |n| u32::try_from(n).unwrap_or(u32::MAX)),
        }
    }
}

/// A marker as a ProjectDoc carries it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Marker {
    pub tc: String,
    pub name: String,
    pub beat: f64,
}

/// The markers in beat order; one whose position does not read is skipped, as `parseMarkers` skips it.
pub fn markers_of(markers: &Markers, sig: TimeSig) -> Vec<Marker> {
    let mut out: Vec<Marker> = markers
        .0
        .iter()
        .filter_map(|(tc, name)| {
            let beat = parse_position(tc, sig).ok()?;
            Some(Marker {
                tc: tc.clone(),
                name: name.clone(),
                beat,
            })
        })
        .collect();
    out.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// The time signature a project's `timeSignature` spells, `4/4` when it says nothing.
pub fn time_sig_of(text: Option<&str>) -> Result<TimeSig, String> {
    let text = text.unwrap_or("4/4");
    let bad = || format!("timeSignature must look like '4/4', got '{text}'");
    let (n, d) = text.split_once('/').ok_or_else(bad)?;
    let part = |s: &str| s.parse::<u32>().ok();
    TimeSig::new(part(n).ok_or_else(bad)?, part(d).ok_or_else(bad)?).map_err(|_| bad())
}

/// The level a root file's raw text declares, with the byte offset of the line that declares it.
pub fn format_of_text(text: &str) -> Option<(u64, usize)> {
    let line_starts = std::iter::once(0).chain(
        text.char_indices()
            .filter_map(|(i, c)| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}').then_some(i + c.len_utf8())),
    );
    for start in line_starts {
        let rest = text.get(start..).unwrap_or("");
        let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
        let Some(rest) = rest.strip_prefix("format") else {
            continue;
        };
        let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
        let Some(rest) = rest.trim_start_matches(js_space).strip_prefix('(') else {
            continue;
        };
        let rest = rest.trim_start_matches(js_space);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return Some((digits.parse().unwrap_or(u64::MAX), start));
        }
    }
    None
}

/// The refusal of a project written by a newer k7, placed on the line that declares the level.
pub fn too_new_finding(file: &str, level: u64, line_at: Option<(&str, usize)>) -> Finding {
    let message = format!(
        "this project was written by a newer k7 (format {level}); this k7 reads format {FORMAT_LEVEL} — update k7 to open it"
    );
    match line_at {
        Some((text, offset)) => Finding::at(file, text, offset, message),
        None => Finding {
            file: file.to_owned(),
            line: 1,
            col: 1,
            message,
            severity: super::finding::Severity::Error,
        },
    }
}

/// Where a project root's own `format` key sits.
pub fn format_key_path() -> Vec<PathSeg> {
    keys(&["format"])
}
