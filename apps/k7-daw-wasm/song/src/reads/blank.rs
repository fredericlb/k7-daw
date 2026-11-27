// reads/blank.rs — the documents a caller with no song folder starts from: a valid, empty project, track, clip and plugin.
// A reader would get wrong that `recordDir: ""` is a REFUSAL rather than a blank: pass a real directory to record anything.

use crate::clips::{ClipDoc, ClipKind};
use crate::doc::{PluginDoc, ProjectDoc, TrackDoc};
use crate::files::{DEFAULT_METRONOME_LEVEL, SettingsDoc};
use serde::Serialize;

/// One of each document with every field at its default, an `id` of `""` and a project named `""`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Blanks {
    pub doc: ProjectDoc,
    pub track: TrackDoc,
    pub clip: ClipDoc,
    pub plugin: PluginDoc,
}

pub fn blanks() -> Blanks {
    let mut doc = ProjectDoc::empty();
    doc.name = String::new();
    // A project that never authored a setting counts no bars in, unlike a compiled one, whose own default is one.
    doc.settings = SettingsDoc { metronome: false, metronome_level: DEFAULT_METRONOME_LEVEL, count_in: 0 };
    Blanks {
        doc,
        track: TrackDoc::new(""),
        clip: ClipDoc {
            id: String::new(),
            kind: ClipKind::Audio,
            name: String::new(),
            source: String::new(),
            start: 0.0,
            length: 0.0,
            offset: 0.0,
            source_from: None,
            source_to: None,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_db: None,
            loop_start: 0.0,
            loop_end: 0.0,
            notes: Vec::new(),
            warp: None,
            warp_off: None,
            disabled: None,
            event: None,
            comp: None,
            crossfade: None,
            nest: None,
        },
        plugin: PluginDoc::new(""),
    }
}
