// ops/picture.rs — the picture.* operations: which frame shape a viewport is, the settings a render line says, and what they resolve to.
// A reader would get wrong that the vocabularies (`ASPECTS`, the formats, the qualities) are not here: they cross once, as `consts.generated.ts`.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_schema::{RenderScene, VideoSettings, aspect_of, parse_video_settings, resolve_video_settings};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ViewportArgs {
    /// Null when the page's number was NaN or infinite: JSON has neither, and such a viewport is none.
    pub w: Option<f64>,
    pub h: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WordsArgs {
    pub words: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolveSettingsArgs {
    pub settings: VideoSettings,
    pub scene: RenderScene,
}

/// A frame shape: what it is called, and the pixels it means.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Aspect {
    pub name: String,
    pub w: f64,
    pub h: f64,
}

/// The picture family's requests: `{"op": "picture.aspect_of", "args": {"w": 1280, "h": 720}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum PictureRequest {
    #[serde(rename = "picture.aspect_of")]
    AspectOf(ViewportArgs),
    #[serde(rename = "picture.parse_settings")]
    ParseSettings(WordsArgs),
    #[serde(rename = "picture.resolve_settings")]
    ResolveSettings(ResolveSettingsArgs),
}

pub fn dispatch(request: PictureRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        PictureRequest::AspectOf(a) => write(
            out,
            &a.w.zip(a.h).and_then(|(w, h)| aspect_of(w, h)).map(|s| Aspect {
                name: s.name.to_owned(),
                w: s.w,
                h: s.h,
            }),
        ),
        PictureRequest::ParseSettings(a) => write(out, &parse_video_settings(&a.words)),
        PictureRequest::ResolveSettings(a) => write(out, &resolve_video_settings(&a.settings, &a.scene)),
    }
}
