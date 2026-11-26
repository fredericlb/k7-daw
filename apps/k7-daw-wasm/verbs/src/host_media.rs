// host_media.rs — the host members the media verbs need, as the front's routes answer them: a probe, a clip's copies, a film's sound, a render.
// A reader would get wrong that a refusal of a route is a throw: each member answers `Refusal`, which the verb words.

use crate::host::{Failed, Host};
use serde_json::{Value as Json, json};

/// The route a film is probed by, and the ones that make its copies and its sound.
const PROBE: &str = "POST /verb-host/probe";
const PREPARE: &str = "POST /verb-host/prepare";
const EXTRACT: &str = "POST /verb-host/extract";

/// The refusal a pending call reads as: the run ends pending whatever the verb does with it, and lands nothing.
fn waiting() -> Refusal {
    Refusal {
        no_ffmpeg: false,
        message: "waiting for the host".to_owned(),
    }
}

impl From<Failed> for Refusal {
    fn from(f: Failed) -> Refusal {
        Refusal {
            no_ffmpeg: f.code.as_deref() == Some("no-ffmpeg"),
            message: f.error,
        }
    }
}

/// ffprobe's facts as the probe route answers them (`k7-mediatek/src/video_probe.rs`).
fn probe_of(f: &Json) -> Probe {
    let text = |k: &str| f.get(k).and_then(Json::as_str).unwrap_or_default().to_owned();
    let num = |k: &str| f.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    Probe {
        codec: text("codec"),
        width: num("width"),
        height: num("height"),
        pix_fmt: text("pixFmt"),
        seconds: num("seconds"),
        container: text("container"),
        rotation: f.get("rotation").and_then(Json::as_f64),
        no_ffmpeg: f.get("noFfmpeg").and_then(Json::as_bool) == Some(true),
    }
}

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

/// What `extractSound` came to: the file's facts, or no sound in the video.
#[derive(Clone, Debug, PartialEq)]
pub enum Extracted {
    Made {
        frames: f64,
        sample_rate: f64,
        channels: f64,
    },
    NoSound,
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
    /// `probe(file)`: a video's facts, `Ok(None)` when it has no picture.
    ///
    /// # Errors
    /// The route's refusal, `no_ffmpeg` when the machine has no ffprobe.
    pub fn probe(&mut self, file: &str) -> Result<Option<Probe>, Refusal> {
        let answer = self.call(PROBE, json!({ "file": file })).map_err(|_| waiting())?;
        let facts = answer.map_err(Refusal::from)?;
        Ok(facts.get("facts").filter(|f| f.is_object()).map(probe_of))
    }

    /// `prepare(ask)`: the copies of one clip, made by the front in the background and landed in the clip file when each is written.
    ///
    /// # Errors
    /// The route's refusal.
    pub fn prepare(&mut self, ask: &PrepareAsk) -> Result<Prepared, Refusal> {
        let args = json!({ "clip": ask.clip, "source": ask.source, "optimized": ask.optimized, "proxy": ask.proxy });
        let answer = self.call(PREPARE, args).map_err(|_| waiting())?;
        let answer = answer.map_err(Refusal::from)?;
        Ok(Prepared {
            started: answer.get("started").and_then(Json::as_bool) != Some(false),
            line: answer.get("line").and_then(Json::as_str).map(str::to_owned),
        })
    }

    /// `extractSound(video, out)`: the sound of a video, written by the front's job and copied to `out` in this run's writes.
    ///
    /// # Errors
    /// The route's refusal, or a file the job made that cannot be copied.
    pub fn extract_sound(&mut self, video: &str, out: &str) -> Result<Extracted, Refusal> {
        let answer = self.call(EXTRACT, json!({ "video": video })).map_err(|_| waiting())?;
        let answer = answer.map_err(Refusal::from)?;
        let Some(facts) = answer.get("facts").filter(|f| f.is_object()) else {
            return Ok(Extracted::NoSound);
        };
        let dir = answer.get("dir").and_then(Json::as_str).unwrap_or_default();
        let made = answer
            .get("files")
            .and_then(Json::as_array)
            .and_then(|f| f.first())
            .and_then(|f| f.get("out"))
            .and_then(Json::as_str);
        let Some(made) = made else {
            return Ok(Extracted::NoSound);
        };
        let fail = |e: song::SourceError| Refusal {
            no_ffmpeg: false,
            message: e.to_string(),
        };
        if let Some((parent, _)) = out.rsplit_once('/') {
            self.fs.mkdir_all(parent).map_err(fail)?;
        }
        self.fs.copy(&format!("{dir}/{made}"), out).map_err(fail)?;
        let num = |k: &str| facts.get(k).and_then(Json::as_f64).unwrap_or(0.0);
        Ok(Extracted::Made {
            frames: num("frames"),
            sample_rate: num("sampleRate"),
            channels: num("channels"),
        })
    }
}
