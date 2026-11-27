// files/mod.rs — the file readers: a .k7x text decoded into a typed value, or its first finding, located.
// A reader would get wrong that a file reports ONE finding, the first: serde stops at the first thing it cannot read.

mod de;
pub mod desk;
mod entity;
mod finding;
mod notes;
mod placement;
pub mod preset_file;
mod project;
pub mod route;
pub(crate) mod scalar;
mod timeline;
pub mod validate;

pub use de::{DeError, from_map};
pub use entity::{Align, AudioClip, CaptionClip, CaptionPosition, Clip, Note, OnsetEdits, Sequence, VideoClip, Warp};
pub use finding::{Finding, Severity, keys, line_col, locate};
pub use notes::{Notes, expand_notes};
pub use placement::{
    AnimKey, Animate, Animated, CompPiece, Crossfade, Ease, Fade, FadeMap, Gain, Pitch, Placement, Take,
};
pub use project::{
    DEFAULT_METRONOME_LEVEL, METRONOME_MIN, FORMAT_LEVEL, Layers, Marker, Markers, ProjectFile, ProjectSettings, RootFile, Settings, SettingsDoc, TooNew,
    Viewport, markers_of, time_sig_of,
};
pub use scalar::{WindowEdge, has_ext};
pub use timeline::{Timeline, TimelineLane};
pub use validate::{ModuleIssue, Validity, ValidityDepth, finding_issue, validate_file};

use k7x::{Bump, MapNode, Node, PathSeg, Scalar, parse_tree};
use serde::de::DeserializeOwned;

/// A refinement the schema makes after the fields read: the key path it is about and its message.
pub trait Check {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        None
    }
}

/// Parses `text`, decodes its root with `read`, then runs the value's own check; the first finding wins.
pub(crate) fn decode_with<T: Check>(
    file: &str,
    text: &str,
    read: impl FnOnce(&MapNode) -> Result<T, DeError>,
) -> Result<T, Finding> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    let value = read(&root).map_err(|e| Finding::at(file, text, e.at.unwrap_or(root.start), e.message))?;
    if let Some((path, message)) = value.check() {
        return Err(Finding::at(
            file,
            text,
            locate(&root, &path).unwrap_or(root.start),
            message,
        ));
    }
    Ok(value)
}

pub(crate) fn decode<T: DeserializeOwned + Check>(file: &str, text: &str) -> Result<T, Finding> {
    decode_with(file, text, from_map::<T>)
}

pub(crate) fn decode_clip(file: &str, text: &str) -> Result<Clip, Finding> {
    decode_with(file, text, entity::decode_clip)
}

/// The level the parsed root declares, when it is a number.
fn data_level(root: &MapNode) -> Option<f64> {
    root.entries
        .iter()
        .rev()
        .find(|e| e.key == "format")
        .and_then(|e| match e.value {
            Node::Scalar {
                value: Scalar::Num(n), ..
            } if n.is_finite() => Some(n),
            _ => None,
        })
}

/// project.k7x: a newer format is refused from the raw text, before any parse, and again from the parsed data.
pub(crate) fn decode_root(file: &str, text: &str) -> Result<RootFile, Finding> {
    let refuse = |level: u64, finding: Finding| RootFile {
        project: None,
        too_new: Some(TooNew { level, finding }),
        time_signature_at: None,
    };
    if let Some((level, at)) = project::format_of_text(text)
        && level > FORMAT_LEVEL
    {
        return Ok(refuse(level, project::too_new_finding(file, level, Some((text, at)))));
    }
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    if let Some(n) = data_level(&root)
        && n > FORMAT_LEVEL as f64
    {
        let level = n as u64;
        let at = locate(&root, &project::format_key_path()).unwrap_or(root.start);
        return Ok(refuse(level, project::too_new_finding(file, level, Some((text, at)))));
    }
    let at = locate(&root, &finding::keys(&["project", "timeSignature"])).map(|o| line_col(text, o));
    decode_with(file, text, from_map::<ProjectFile>).map(|p| RootFile {
        project: Some(p),
        too_new: None,
        time_signature_at: at,
    })
}
