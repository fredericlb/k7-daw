// render_settings.rs — the words that set a picture render, read in ONE place: `project export video` and `picture render` take the same line.
// A reader would get wrong that a word's value is whatever follows its first `=`: `size=720p=x` is refused, not read as `720p`.

use serde::{Deserialize, Serialize};

/// The quality presets an export may ask for, lowest first; `max` is mediabunny's "very high".
pub const EXPORT_QUALITIES: [&str; 4] = ["low", "medium", "high", "max"];

/// The containers an export leaves in, the default first; the picture leaves in the same two.
pub const EXPORT_FORMATS: [&str; 2] = ["webm", "mp4"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ExportQuality {
    Low,
    Medium,
    High,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Webm,
    Mp4,
}

/// A size in px: a height alone takes the viewport's aspect, a width and a height are both stated.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VideoSize {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub width: Option<f64>,
    pub height: f64,
}

/// The render's own settings, each optional: what a line leaves out is the viewport's, `high`, the whole song.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VideoSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub size: Option<VideoSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub quality: Option<ExportQuality>,
    /// Seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<f64>,
    /// Seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub to: Option<f64>,
}

/// The scene a render's settings resolve against.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RenderScene {
    pub viewport: [f64; 2],
    pub fps: f64,
    pub duration_sec: f64,
}

/// The range and the file size the settings resolve to: `size` is absent when the settings state none.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedVideo {
    pub from: f64,
    pub frame_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub size: Option<[f64; 2]>,
}

/// The range and file size `settings` resolve to on a scene: the whole song by default, a height alone taking the viewport's aspect.
#[must_use]
pub fn resolve_video_settings(settings: &VideoSettings, scene: &RenderScene) -> ResolvedVideo {
    let from = settings.from.unwrap_or(0.0);
    let to = settings.to.unwrap_or(scene.duration_sec);
    let frame_count = ((to - from) * scene.fps).round().max(1.0);
    let [vw, vh] = scene.viewport;
    let size = settings.size.map(|s| match s.width {
        Some(w) => [w, s.height],
        None => [(s.height * vw / vh).round(), s.height],
    });
    ResolvedVideo {
        from,
        frame_count,
        size,
    }
}

/// Seconds, or `m:ss(.s)`: the picture's own spelling of a time.
fn parse_clock(s: &str) -> Option<f64> {
    let (minutes, secs) = match s.split_once(':') {
        Some((m, rest)) => (Some(m), rest),
        None => (None, s),
    };
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let decimal = match secs.split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => digits(secs),
    };
    if !decimal || minutes.is_some_and(|m| !digits(m)) {
        return None;
    }
    let sec = secs.parse::<f64>().ok()?;
    let min = minutes.map_or(Some(0.0), |m| m.parse::<f64>().ok())?;
    Some(sec + min * 60.0).filter(|t| t.is_finite())
}

fn number(x: &str) -> Option<f64> {
    (!x.is_empty() && x.bytes().all(|b| b.is_ascii_digit())).then(|| x.parse().ok())?
}

// One `key=value` word into `into`; false when it is not one this understands.
fn setting(word: &str, into: &mut VideoSettings) -> bool {
    let (key, value) = word.split_once('=').unwrap_or((word, ""));
    match key {
        "size" => {
            if let Some(h) = value.strip_suffix('p').and_then(number) {
                into.size = Some(VideoSize { width: None, height: h });
            } else if let Some((w, h)) = value.split_once('x')
                && let (Some(w), Some(h)) = (number(w), number(h))
            {
                into.size = Some(VideoSize {
                    width: Some(w),
                    height: h,
                });
            } else {
                return false;
            }
            true
        }
        "quality" => {
            into.quality = match value {
                "low" => Some(ExportQuality::Low),
                "medium" => Some(ExportQuality::Medium),
                "high" => Some(ExportQuality::High),
                "max" => Some(ExportQuality::Max),
                _ => return false,
            };
            true
        }
        "from" | "to" => {
            let Some(t) = parse_clock(value) else {
                return false;
            };
            if key == "from" {
                into.from = Some(t);
            } else {
                into.to = Some(t);
            }
            true
        }
        _ => false,
    }
}

/// The settings `words` say, or None when one is not understood or the range runs backwards.
#[must_use]
pub fn parse_video_settings<S: AsRef<str>>(words: &[S]) -> Option<VideoSettings> {
    let mut out = VideoSettings::default();
    for w in words {
        if !setting(&w.as_ref().to_lowercase(), &mut out) {
            return None;
        }
    }
    match (out.from, out.to) {
        (Some(f), Some(t)) if t <= f => None,
        _ => Some(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(words: &[&str]) -> Option<VideoSettings> {
        parse_video_settings(words)
    }

    #[test]
    fn a_line_leaves_out_what_it_does_not_say() {
        assert_eq!(read(&[]), Some(VideoSettings::default()));
        let s = read(&["size=720p", "QUALITY=Max", "from=1:30", "to=2:00.5"]).unwrap();
        assert_eq!(
            s.size,
            Some(VideoSize {
                width: None,
                height: 720.0
            })
        );
        assert_eq!(s.quality, Some(ExportQuality::Max));
        assert_eq!((s.from, s.to), (Some(90.0), Some(120.5)));
        let wh = read(&["size=1280x720"]).unwrap();
        assert_eq!(
            wh.size,
            Some(VideoSize {
                width: Some(1280.0),
                height: 720.0
            })
        );
    }

    #[test]
    fn a_word_it_does_not_understand_refuses_the_line() {
        for bad in [
            "size=big",
            "size=720",
            "size=720p=x",
            "quality=ultra",
            "from=a",
            "from=1:2:3",
            "fps=30",
            "size",
        ] {
            assert_eq!(read(&[bad]), None, "{bad}");
        }
    }

    #[test]
    fn a_range_that_runs_backwards_is_refused() {
        assert_eq!(read(&["from=10", "to=10"]), None);
        assert_eq!(read(&["from=10", "to=5"]), None);
        assert!(read(&["from=5", "to=10"]).is_some());
        assert!(read(&["to=5"]).is_some());
    }

    #[test]
    fn a_range_and_a_size_resolve_against_the_scene() {
        let scene = RenderScene {
            viewport: [1920.0, 1080.0],
            fps: 30.0,
            duration_sec: 60.0,
        };
        let whole = resolve_video_settings(&VideoSettings::default(), &scene);
        assert_eq!((whole.from, whole.frame_count, whole.size), (0.0, 1800.0, None));
        let s = read(&["size=720p", "from=10", "to=20"]).unwrap();
        let r = resolve_video_settings(&s, &scene);
        assert_eq!((r.from, r.frame_count, r.size), (10.0, 300.0, Some([1280.0, 720.0])));
        let wh = read(&["size=640x480"]).unwrap();
        assert_eq!(resolve_video_settings(&wh, &scene).size, Some([640.0, 480.0]));
        let empty = RenderScene {
            duration_sec: 0.0,
            ..scene
        };
        assert_eq!(
            resolve_video_settings(&VideoSettings::default(), &empty).frame_count,
            1.0
        );
    }
}
