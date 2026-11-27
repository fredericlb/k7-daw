// doc.rs — the picture document the page reads (web-protocol's `WebVideoProject`), every path absolute until a caller maps it.
// A reader would get wrong which fields are absent and which are empty: the page tells "no fade" from a zero one by absence.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value as Json;

fn no(b: &bool) -> bool {
    !*b
}

/// The type of a still shot's flag: only `true` is ever written.
#[cfg(feature = "ts")]
#[derive(ts_rs::TS)]
#[ts(type = "true", rename = "WebVideoStill")]
struct Still;

pub use eyes::{CaptionStyle, Captions, Fade, Key, Transition};

/// One shot; `src` is empty on an activation and on a shot whose file is not there.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoClip", optional_fields))]
pub struct Clip {
    pub id: String,
    pub name: String,
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(skip_serializing_if = "no")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Still>"))]
    pub still: bool,
    pub start: f64,
    pub length: f64,
    pub source_from: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<Fade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<Fade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<std::collections::HashMap<String, Vec<Key>>>"))]
    pub animate: Option<IndexMap<String, Vec<Key>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captions: Option<Captions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<String>,
}

/// A shader's two stages, read off its folder.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoGlsl"))]
pub struct Glsl {
    pub name: String,
    pub vertex: String,
    pub fragment: String,
}

/// One pass over the picture: a shader with its GLSL inline, or a named video effect.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoPlugin", optional_fields))]
pub struct Plugin {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(type = "\"video_effect\" | \"shader\""))]
    pub kind: &'static str,
    pub name: String,
    pub enabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
    pub params: IndexMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<Glsl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut: Option<String>,
}

/// A picture lane in draw order: its rect, its shots, its shapes, its run and its lens.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoTrack", optional_fields))]
pub struct Track {
    pub id: String,
    pub name: String,
    pub color: String,
    pub mute: bool,
    pub solo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "\"off\" | \"auto\" | \"on\"", optional))]
    pub camera_monitor: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub clips: Vec<Clip>,
    #[cfg_attr(feature = "ts", ts(as = "Vec<eyes::scene::SceneShape>"))]
    pub shapes: Vec<Json>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Plugin>>"))]
    pub plugins: Vec<Plugin>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoMarker"))]
pub struct Marker {
    pub sec: f64,
    pub name: String,
    pub tc: String,
}

/// The picture of a project, as `POST /picture` answers it once its paths are made workspace-relative.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebVideoProject", optional_fields))]
pub struct Doc {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_order: Option<Vec<String>>,
    #[cfg_attr(feature = "ts", ts(as = "(f64, f64)"))]
    pub viewport: [f64; 2],
    pub fps: f64,
    pub duration: f64,
    pub markers: Vec<Marker>,
    pub video_tracks: Vec<Track>,
    pub frame_plugins: Vec<Plugin>,
    #[cfg_attr(feature = "ts", ts(type = "\"mix\" | null"))]
    pub sound: Option<&'static str>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, import(\"./SceneTransition\").SceneTransition>"))]
    pub transitions: IndexMap<String, Transition>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, Array<import(\"./WebVideoPlugin\").WebVideoPlugin>>"))]
    pub stretches: IndexMap<String, Vec<Plugin>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, Array<import(\"./WebVideoTrack\").WebVideoTrack>>", optional))]
    pub timelines: Option<IndexMap<String, Vec<Track>>>,
}

fn map_run(run: &mut [Plugin], f: &dyn Fn(&str) -> String) {
    for p in run {
        if let Some(l) = &p.lut {
            p.lut = Some(f(l));
        }
    }
}

fn map_tracks(tracks: &mut [Track], f: &dyn Fn(&str) -> String) {
    for t in tracks {
        for c in &mut t.clips {
            if !c.src.is_empty() {
                c.src = f(&c.src);
            }
            c.optimized = c.optimized.as_deref().map(f);
            c.proxy = c.proxy.as_deref().map(f);
        }
        map_run(&mut t.plugins, f);
    }
}

impl Doc {
    /// The picture of a song that has none: never a failure, since every song may carry one.
    pub fn empty(name: &str) -> Doc {
        Doc {
            name: name.to_owned(),
            ..Doc::default()
        }
    }

    /// Every media and LUT path passed through `f`: the page's workspace-relative paths, or a render's own.
    pub fn map_paths(&mut self, f: &dyn Fn(&str) -> String) {
        map_tracks(&mut self.video_tracks, f);
        map_run(&mut self.frame_plugins, f);
        for run in self.stretches.values_mut() {
            map_run(run, f);
        }
        for lanes in self.timelines.iter_mut().flat_map(IndexMap::values_mut) {
            map_tracks(lanes, f);
        }
    }

    /// Where the last picture ends, in seconds; a length-0 shot gives its start alone.
    pub fn duration_of(tracks: &[Track]) -> f64 {
        tracks
            .iter()
            .flat_map(|t| &t.clips)
            .fold(0.0, |end, c| super::js_max(end, c.start + super::js_max(0.0, c.length)))
    }
}
