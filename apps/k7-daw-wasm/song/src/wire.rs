// wire.rs — the small shapes a file-service route answers and the page reads, with the words they are spelled from.
// A reader would get wrong that nothing here validates a file: these are answers, whose types the page takes from ts-rs.

use serde::{Deserialize, Serialize};

/// The archetypes a project is created as: three named after the surface they open on, the two `live` ones after what they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ProjectShape {
    Arrangement,
    Mix,
    Video,
    Live,
    LiveVideo,
}

/// Every archetype as a project file spells it, in the order the new-project menu lists them.
pub const PROJECT_SHAPES: [&str; 5] = ["arrangement", "mix", "video", "live", "live-video"];

/// One preset as a listing names it: where it is kept (`workspace`, or a library's name) and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PresetEntry {
    pub scope: String,
    pub name: String,
}

/// What a node offers: the module ref its presets are keyed by, those presets, and where a new one may go.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NodePresets {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub reference: Option<String>,
    pub presets: Vec<PresetEntry>,
    pub scopes: Vec<String>,
}

/// A reference track as a listing hands it over: its path is workspace-relative, so it is what `/media` takes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReferenceTrack {
    pub path: String,
    /// The file name without its extension, which is what a row reads as.
    pub name: String,
    /// Bytes on disk, the one thing a listing can say about a file nobody has decoded yet.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub bytes: u64,
}

/// The extensions offered as reference tracks: what a browser's `<audio>` can be asked to decode.
pub const REFERENCE_EXTS: [&str; 10] = ["wav", "flac", "mp3", "m4a", "aac", "ogg", "opus", "aif", "aiff", "webm"];

/// The extensions a clip's `source` names as a picture, a still and a caption file: what makes a clip file say only `source:`.
pub const VIDEO_EXTS: &[&str] = crate::files::scalar::VIDEO_EXTS;
pub const IMAGE_EXTS: &[&str] = crate::files::scalar::IMAGE_EXTS;
pub const CAPTION_EXTS: &[&str] = crate::files::scalar::CAPTION_EXTS;

/// A name ending in `.k7x` is a workspace document; the extension as a pattern source, for a regex.
pub const DOC_EXT_SRC: &str = ".k7x";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shape_list_is_the_enum_spelled_as_a_file_spells_it() {
        let shapes = [
            ProjectShape::Arrangement,
            ProjectShape::Mix,
            ProjectShape::Video,
            ProjectShape::Live,
            ProjectShape::LiveVideo,
        ]
        .map(|s| serde_json::to_value(s).unwrap());
        assert_eq!(shapes.map(|v| v.as_str().unwrap().to_owned()), PROJECT_SHAPES);
    }

    #[test]
    fn a_node_s_presets_answer_with_the_key_ref() {
        let p = NodePresets {
            reference: None,
            presets: vec![PresetEntry {
                scope: "workspace".into(),
                name: "a".into(),
            }],
            scopes: vec![],
        };
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"ref":null,"presets":[{"scope":"workspace","name":"a"}],"scopes":[]}"#
        );
    }
}
