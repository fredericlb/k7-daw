// export.rs — what `project export` was asked for: the four shapes of its line, and the render settings words both ends read alike.
// A reader would get wrong that the bare `project export wav` still works: it is refused, or `project export webm` would mean the mixdown.

use serde::Serialize;
use serde_json::{Map, Value, json};
use song_edit::js_text::is_js_space;

/// The mixdown formats the file service renders.
pub const MIX_FORMATS: [&str; 2] = ["wav", "mp3"];
/// The containers the picture leaves in, the default first.
pub const VIDEO_FORMATS: [&str; 2] = ["webm", "mp4"];
/// The cut handed to another editor: OpenTimelineIO, or a CMX 3600 EDL of one lane.
pub const TIMELINE_FORMATS: [&str; 2] = ["otio", "edl"];
/// The quality presets an export may ask for, lowest first.
pub const EXPORT_QUALITIES: [&str; 4] = ["low", "medium", "high", "max"];
/// What the audio of an exported video is rendered from when the line names none: it is re-encoded either way, so the lossy step buys nothing.
pub const DEFAULT_VIDEO_AUDIO: &str = "wav";
/// The timeline word that means the song itself.
const MAIN_TIMELINE: &str = "main";

/// The usage line, built from the vocabularies above so it cannot drift from them.
#[must_use]
pub fn usage() -> String {
    format!(
        "project export audio <{mix}> [timeline] \u{b7} project export video <{video}> [<{mix}>|none] [size=720p|1280x720] [quality={q}] [from=<t>] [to=<t>] \u{b7} project export timeline <{tl}> [lane] \u{b7} project export tracks [timeline]",
        mix = MIX_FORMATS.join("|"),
        video = VIDEO_FORMATS.join("|"),
        q = EXPORT_QUALITIES.join("|"),
        tl = TIMELINE_FORMATS.join("|"),
    )
}

/// The command line the share panel runs for the tracks zip.
#[must_use]
pub fn tracks_line(timeline: Option<&str>) -> String {
    match timeline.filter(|t| !t.is_empty()) {
        Some(t) => format!("project export tracks {t}"),
        None => "project export tracks".to_owned(),
    }
}

/// A mixdown format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum MixFormat {
    Wav,
    Mp3,
}

impl MixFormat {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            _ => None,
        }
    }
}

/// A picture container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum VideoFormat {
    Webm,
    Mp4,
}

impl VideoFormat {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "webm" => Some(Self::Webm),
            "mp4" => Some(Self::Mp4),
            _ => None,
        }
    }
}

/// An interchange format for the cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum TimelineFormat {
    Otio,
    Edl,
}

impl TimelineFormat {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "otio" => Some(Self::Otio),
            "edl" => Some(Self::Edl),
            _ => None,
        }
    }
}

/// What the line asks to leave the project as.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ExportAsk {
    /// The mixdown: one timeline alone as it is auditioned, absent or `main` meaning the song.
    Audio {
        format: MixFormat,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        timeline: Option<String>,
    },
    /// The frame, rendered here; `audio` is null only for a silent film.
    Video {
        format: VideoFormat,
        audio: Option<MixFormat>,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
        settings: Map<String, Value>,
    },
    /// The cut, for another editor to open.
    Timeline {
        format: TimelineFormat,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        lane: Option<String>,
    },
    /// Every track as a zip: audio without effects, MIDI and a page.
    Tracks {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        timeline: Option<String>,
    },
}

/// A timeline's id as its filename is spelled.
fn is_timeline_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn is_mix(s: &str) -> bool {
    MixFormat::parse(s).is_some()
}

/// Seconds, or `m:ss(.s)`: the picture's own spelling of a time.
fn clock(s: &str) -> Option<f64> {
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let decimal = |x: &str| match x.split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => digits(x),
    };
    let (minutes, seconds) = match s.split_once(':') {
        Some((m, rest)) => (Some(m), rest),
        None => (None, s),
    };
    if !decimal(seconds) || minutes.is_some_and(|m| !digits(m)) {
        return None;
    }
    let sec: f64 = seconds.parse().ok()?;
    let min: f64 = minutes.map_or(Some(0.0), |m| m.parse().ok())?;
    Some(sec + min * 60.0)
}

/// A whole number as JSON, so `720` is not written `720.0`.
fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

/// One `key=value` word into `into`; false when it is not one this understands.
fn setting(word: &str, into: &mut Map<String, Value>) -> bool {
    // A second `=` is cut off with what follows it, as a split limited to two did.
    let mut parts = word.split('=');
    let key = parts.next().unwrap_or("");
    let value = parts.next().unwrap_or("");
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    match key {
        "size" => {
            if let Some(h) = value.strip_suffix('p').filter(|h| digits(h)) {
                into.insert("size".to_owned(), json!({ "height": number(h.parse().unwrap_or(0.0)) }));
            } else if let Some((w, h)) = value.split_once('x').filter(|(w, h)| digits(w) && digits(h)) {
                into.insert(
                    "size".to_owned(),
                    json!({ "width": number(w.parse().unwrap_or(0.0)), "height": number(h.parse().unwrap_or(0.0)) }),
                );
            } else {
                return false;
            }
            true
        }
        "quality" => {
            if !EXPORT_QUALITIES.contains(&value) {
                return false;
            }
            into.insert("quality".to_owned(), json!(value));
            true
        }
        "from" | "to" => {
            let Some(t) = clock(value) else { return false };
            into.insert(key.to_owned(), number(t));
            true
        }
        _ => false,
    }
}

/// The render settings `words` say, or none when one is not understood or the range runs backwards.
#[must_use]
pub fn parse_video_settings(words: &[String]) -> Option<Map<String, Value>> {
    let mut out = Map::new();
    for w in words {
        if !setting(&w.to_lowercase(), &mut out) {
            return None;
        }
    }
    let (from, to) = (
        out.get("from").and_then(Value::as_f64),
        out.get("to").and_then(Value::as_f64),
    );
    if let (Some(f), Some(t)) = (from, to)
        && t <= f
    {
        return None;
    }
    Some(out)
}

/// The rest of a `project export` line, or none when it says nothing this understands.
#[must_use]
pub fn parse_export_ask(rest: &str) -> Option<ExportAsk> {
    let raw: Vec<&str> = rest.split(is_js_space).filter(|s| !s.is_empty()).collect();
    // Only the first two words are case-folded: a lane or a timeline keeps its own spelling.
    let parts: Vec<String> = raw
        .iter()
        .enumerate()
        .map(|(i, p)| if i < 2 { p.to_lowercase() } else { (*p).to_owned() })
        .collect();
    let what = parts.first().map(String::as_str)?;
    let format = parts.get(1).map(String::as_str);
    let tail = parts.get(2..).unwrap_or(&[]);
    match what {
        "audio" => {
            let format = format.and_then(MixFormat::parse)?;
            if tail.len() > 1 {
                return None;
            }
            // A third word is a timeline's id, never a second format, which is a slip rather than a name.
            let word = tail.first();
            if let Some(w) = word
                && (is_mix(&w.to_lowercase()) || !is_timeline_id(w))
            {
                return None;
            }
            let timeline = word.filter(|w| w.as_str() != MAIN_TIMELINE).cloned();
            Some(ExportAsk::Audio { format, timeline })
        }
        "tracks" => {
            // `tracks` has no format word: the second word is a timeline, which the case-fold above took for one.
            if !tail.is_empty() || format.is_some_and(|w| !is_timeline_id(w)) {
                return None;
            }
            let timeline = format
                .filter(|w| *w != MAIN_TIMELINE)
                .and_then(|_| raw.get(1).map(|s| (*s).to_owned()));
            Some(ExportAsk::Tracks { timeline })
        }
        "timeline" => {
            let format = format.and_then(TimelineFormat::parse)?;
            if tail.len() > 1 {
                return None;
            }
            Some(ExportAsk::Timeline {
                format,
                lane: tail.first().cloned(),
            })
        }
        "video" => {
            let format = format.and_then(VideoFormat::parse)?;
            // The sound word, when there is one, comes first; every word after it is a `key=value`.
            let first = tail.first().map(|t| t.to_lowercase());
            let named = first.as_ref().is_some_and(|f| !f.contains('='));
            let audio = match (&first, named) {
                (Some(f), true) if f == "none" => None,
                (Some(f), true) => Some(MixFormat::parse(f)?),
                _ => Some(MixFormat::Wav),
            };
            let settings = parse_video_settings(tail.get(usize::from(named)..).unwrap_or(&[]))?;
            Some(ExportAsk::Video {
                format,
                audio,
                settings,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(rest: &str) -> Option<Value> {
        parse_export_ask(rest).map(|a| serde_json::to_value(a).unwrap())
    }

    #[test]
    fn audio_takes_each_mixdown_format_and_refuses_the_rest() {
        assert_eq!(ask("audio wav"), Some(json!({ "kind": "audio", "format": "wav" })));
        assert_eq!(ask("audio mp3"), Some(json!({ "kind": "audio", "format": "mp3" })));
        assert_eq!(ask("audio flac"), None);
        assert_eq!(ask("audio"), None);
        assert_eq!(ask("audio wav mp3"), None);
    }

    #[test]
    fn audio_takes_one_timeline_and_main_is_the_song_itself() {
        assert_eq!(
            ask("audio wav verse"),
            Some(json!({ "kind": "audio", "format": "wav", "timeline": "verse" }))
        );
        assert_eq!(ask("audio mp3 main"), Some(json!({ "kind": "audio", "format": "mp3" })));
        assert_eq!(ask("audio wav verse chorus"), None);
        assert_eq!(ask("audio wav ver/se"), None);
    }

    #[test]
    fn video_is_rendered_with_sound_unless_it_says_none() {
        let silent = json!({ "kind": "video", "format": "webm", "audio": null, "settings": {} });
        assert_eq!(
            ask("video webm"),
            Some(json!({ "kind": "video", "format": "webm", "audio": "wav", "settings": {} }))
        );
        assert_eq!(
            ask("video mp4 mp3"),
            Some(json!({ "kind": "video", "format": "mp4", "audio": "mp3", "settings": {} }))
        );
        assert_eq!(ask("video webm none"), Some(silent));
        for format in VIDEO_FORMATS {
            assert!(ask(&format!("video {format}")).is_some());
        }
        assert_eq!(ask("video mov"), None);
        assert_eq!(ask("video"), None);
        assert_eq!(ask("video webm flac"), None);
    }

    #[test]
    fn the_bare_old_line_is_refused_rather_than_read_as_audio() {
        for line in ["wav", "mp3", "webm", ""] {
            assert_eq!(ask(line), None, "{line:?}");
        }
    }

    #[test]
    fn the_line_is_not_fussy_about_case_or_spacing() {
        assert_eq!(
            ask("  VIDEO   MP4   MP3 "),
            Some(json!({ "kind": "video", "format": "mp4", "audio": "mp3", "settings": {} }))
        );
    }

    #[test]
    fn the_render_settings_are_read_after_the_sound_or_without_it() {
        let got = ask("video mp4 mp3 size=720p quality=max from=1:30 to=1:45.5").unwrap();
        assert_eq!(got["audio"], "mp3");
        assert_eq!(
            got["settings"],
            json!({ "size": { "height": 720 }, "quality": "max", "from": 90, "to": 105.5 })
        );
        let got = ask("video webm size=1280x720").unwrap();
        assert_eq!(got["audio"], "wav");
        assert_eq!(got["settings"], json!({ "size": { "width": 1280, "height": 720 } }));
        assert_eq!(ask("video mp4 quality=ultra"), None);
        assert_eq!(ask("video mp4 fps=60"), None);
        assert_eq!(ask("video mp4 from=10 to=5"), None);
    }

    #[test]
    fn the_cut_names_its_format_and_for_an_edl_the_lane() {
        assert_eq!(
            ask("timeline otio"),
            Some(json!({ "kind": "timeline", "format": "otio" }))
        );
        assert_eq!(
            ask("timeline edl broll"),
            Some(json!({ "kind": "timeline", "format": "edl", "lane": "broll" }))
        );
        assert_eq!(ask("timeline aaf"), None);
    }

    #[test]
    fn the_tracks_zip_takes_a_timeline_in_its_own_spelling() {
        assert_eq!(ask("tracks"), Some(json!({ "kind": "tracks" })));
        assert_eq!(tracks_line(None), "project export tracks");
        let line = tracks_line(Some("Verse"));
        assert_eq!(
            ask(line.trim_start_matches("project export ")),
            Some(json!({ "kind": "tracks", "timeline": "Verse" }))
        );
        assert_eq!(ask("tracks main"), Some(json!({ "kind": "tracks" })));
        assert_eq!(ask("tracks a b"), None);
        assert_eq!(ask("tracks ver/se"), None);
    }

    #[test]
    fn a_range_that_runs_backwards_or_a_word_unknown_is_no_setting() {
        let w = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(parse_video_settings(&w(&["from=10", "to=5"])).is_none());
        assert!(parse_video_settings(&w(&["quality=max", "from=1:30", "size=1280x720"])).is_some());
        assert!(parse_video_settings(&w(&["fast"])).is_none());
    }

    #[test]
    fn the_usage_line_is_built_from_the_vocabularies() {
        let u = usage();
        assert!(u.contains("video <webm|mp4>"));
        assert!(u.contains("quality=low|medium|high|max"));
    }
}
