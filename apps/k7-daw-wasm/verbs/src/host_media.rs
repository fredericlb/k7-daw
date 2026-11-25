// host_media.rs — the host members the media verbs need and the crate cannot hold: `probe`, `prepare`, `extractSound`, `renderPicture`, `background`.
// A reader would get wrong that an ask answers: it only queues the work, so the verb's line says what was asked, never what came of it.

use crate::host::Host;
use serde_json::{Value as Json, json};

/// A host member the crate cannot answer: `no_ffmpeg` says the tool behind it is absent here, which a verb may carry on without.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub no_ffmpeg: bool,
    pub message: String,
}

/// A video as the plan reads it; `codec` is ffprobe's name and `pix_fmt` says the bit depth and the chroma.
#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub codec: String,
    pub width: f64,
    pub height: f64,
    pub pix_fmt: String,
    pub seconds: f64,
    pub container: String,
    pub rotation: Option<f64>,
    /// Read from the boxes with no ffmpeg: nothing can be made from this file on this host.
    pub no_ffmpeg: bool,
}

/// What `extractSound` came to: the file's facts, no sound in the video, or the work asked of the caller.
#[derive(Clone, Debug, PartialEq)]
pub enum Extracted {
    Made {
        frames: f64,
        sample_rate: f64,
        channels: f64,
    },
    NoSound,
    Asked,
}

/// One clip's copies, as the caller makes them: `optimized` is `remux`, `transcode` or none.
#[derive(Clone, Debug, PartialEq)]
pub struct PrepareAsk {
    pub clip: String,
    pub source: String,
    pub optimized: Option<&'static str>,
    pub proxy: bool,
}

/// What `prepare` answers: `started` is false when that source is already being prepared; `line` is a host that ran it inline saying so.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    pub started: bool,
    pub line: Option<String>,
}

impl Host<'_> {
    /// `probe(file)`: a video's facts, `Ok(None)` when it has no picture; this host reads no video, so it is a refusal without ffmpeg.
    ///
    /// # Errors
    /// A host that cannot read the file.
    pub fn probe(&mut self, _file: &str) -> Result<Option<Probe>, Refusal> {
        Err(Refusal {
            no_ffmpeg: true,
            message: "this host reads no video.".to_owned(),
        })
    }

    /// `prepare(ask)`: the copies of one clip, made by the caller in the background.
    ///
    /// # Errors
    /// A host that makes no copies.
    pub fn prepare(&mut self, ask: &PrepareAsk) -> Result<Prepared, Refusal> {
        let at = self.song_path.clone();
        self.ask(
            "prepare",
            json!({ "at": at, "clip": ask.clip, "source": ask.source, "optimized": ask.optimized, "proxy": ask.proxy }),
        );
        Ok(Prepared {
            started: true,
            line: None,
        })
    }

    /// `extractSound(video, out)`: the sound of a video written to `out` by the caller.
    ///
    /// # Errors
    /// A host that extracts no sound.
    pub fn extract_sound(&mut self, video: &str, out: &str) -> Result<Extracted, Refusal> {
        self.ask("extractSound", json!([video, out]));
        Ok(Extracted::Asked)
    }

    /// `renderPicture(ask)`: the picture rendered by the caller, which builds the scene from the project as it lands.
    ///
    /// # Errors
    /// A host that renders no picture.
    pub fn render_picture(&mut self, ask: Json) -> Result<(), Refusal> {
        self.ask("renderPicture", ask);
        Ok(())
    }

    /// `background(tag, work)`: whether the work a tag names runs in the background, which an asked member always does.
    pub fn background(&mut self, _tag: &str) -> bool {
        true
    }
}
