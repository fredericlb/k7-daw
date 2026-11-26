// model.rs — the neutral song shape every mapper produces and the emitter consumes.
// What a reader gets wrong: it is not any one format's model; both mappers project onto it (docs/import.md).

use std::sync::Arc;

/// Bytes shared between the places that hold them: a sample named by nine clips is read once.
pub type Bytes = Arc<Vec<u8>>;

#[derive(Clone, Debug, PartialEq)]
pub struct NoteOut {
    pub time: f64,
    pub duration: f64,
    pub key: f64,
    pub vel: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipKind {
    Audio,
    Midi,
}

/// A clip's warp: markers map a TIMELINE beat to a SOURCE second.
#[derive(Clone, Debug, PartialEq)]
pub struct WarpOut {
    pub algorithm: Option<String>,
    pub markers: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipOut {
    pub kind: ClipKind,
    pub source: Option<String>,
    pub start: f64,
    pub length: f64,
    pub offset: Option<f64>,
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
    pub disabled: bool,
    pub notes: Vec<NoteOut>,
    pub warp: Option<WarpOut>,
}

impl ClipOut {
    /// A clip of `kind` at `start` for `length` beats, everything else unset.
    #[must_use]
    pub fn new(kind: ClipKind, start: f64, length: f64) -> ClipOut {
        ClipOut {
            kind,
            source: None,
            start,
            length,
            offset: None,
            fade_in: None,
            fade_out: None,
            loop_start: None,
            loop_end: None,
            disabled: false,
            notes: Vec::new(),
            warp: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SendOut {
    pub to: String,
    pub level: f64,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginOut {
    /// "builtin", "vst3", "vst2", "clap", "au".
    pub kind: String,
    pub name: String,
    /// "instrument", "audioFX" or "noteFX".
    pub role: Option<String>,
    pub enabled: bool,
    /// The asset key of its state, when it has one.
    pub path: Option<String>,
    pub params: Vec<(String, f64)>,
    pub params_complete: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackOut {
    pub id: String,
    pub name: String,
    /// "regular", "effect" or "master".
    pub role: Option<String>,
    pub color: Option<String>,
    pub mute: bool,
    pub plugins: Vec<PluginOut>,
    pub sends: Vec<SendOut>,
    pub clips: Vec<ClipOut>,
    /// Its instrument was a drum rack the source DAW alone can play.
    pub drums: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub name: String,
    pub tempo: f64,
    pub sample_rate: f64,
    pub time_signature: String,
    pub tracks: Vec<TrackOut>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub tracks: usize,
    pub audio_clips: usize,
    pub midi_clips: usize,
    pub notes: usize,
    pub assets: usize,
}

/// What a mapper returns: the song, the asset bytes by archive path, what it counted and what it could not carry.
pub struct Translation {
    pub song: Song,
    pub files: Vec<(String, Bytes)>,
    pub stats: Stats,
    pub warnings: Vec<String>,
}
