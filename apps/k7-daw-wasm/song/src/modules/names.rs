// modules/names.rs — the names the page reads from the module vocabulary: nodes, type folders, params, library folders.
// A reader would get wrong that these are only the page's copy: the crate's own code keeps its private spelling of some, and consts.rs prints this list.

pub use super::files::{FILE_PARAMS, LIBRARY_FILE_FAMILIES, LIBRARY_FILE_PARAMS, SHARED_FILE_PARAMS, TAP_SAMPLE};
pub use super::types::{
    COLD_TYPE, CONSOLE_TYPE, DOC_CATEGORY, DOC_REF, ENGINE_NS, META_NS, MODULE_STATUSES, NODE_SEP,
    NOT_SUPPORTED_CATEGORY, NOT_SUPPORTED_REF, NOT_SUPPORTED_TYPE, SHARED_MODULES_DIR, TRACK_CATEGORY, TRACK_REFS,
};
pub use crate::clips::{FIXED_FIELDS, NEST_SEP};
pub use crate::doc::{INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM};
pub use crate::lanes::{CUE_NODE, MASTER_OWNER, OUT_NODE, VIDEO_OUT_NODE};
pub use crate::modules::assets::MODULE_SOURCES_DIR;
pub use crate::routing::{MIX_NODE, RENDER_NODE};

/// The audio node an audio input is: `@meta/IN`, a channel of the interface.
pub const AUDIO_IN_NODE: &str = "IN";
/// The node a MIDI device is: `@meta/MIDI`.
pub const MIDI_IN_NODE: &str = "MIDI";
/// The node a camera is: `@meta/CAM`.
pub const CAMERA_NODE: &str = "CAM";
/// The io kind a live picture source is.
pub const CAMERA_IO: &str = "camera";
/// The io kind a MIDI device is.
pub const MIDI_IN_IO: &str = "midi";

/// RENDER's second input: the sound the picture leaves with.
pub const RENDER_AUDIO_PORT: &str = "audio";

/// The library an account's first repository is named.
pub const DEFAULT_LIBRARY: &str = "k7-core";
/// The account's own library, `@account/…`.
pub const ACCOUNT_LIBRARY: &str = "account";
/// The two fixed levels a module can live at.
pub const FIXED_NAMESPACES: [&str; 2] = ["engine", "workspace"];

/// The folder the account's libraries answer to in a workspace's file API; virtual.
pub const LIBRARIES_DIR: &str = "_libraries";
/// Where the file service clones the libraries, under the machine's state.
pub const LIBRARIES_HOME_DIR: &str = "libraries";
/// Where automatic VST renders are cached, under the machine's state.
pub const AUTO_RENDERS_HOME_DIR: &str = ".renders";
/// Where every project's undo journal is kept, under the machine's state.
pub const HISTORY_HOME_DIR: &str = ".history";

/// A shader module's folder of GLSL, and the two files in it.
pub const SHADERS_DIR: &str = "shaders";
pub const SHADER_VERTEX: &str = "vertex.glsl";
pub const SHADER_FRAGMENT: &str = "fragment.glsl";
/// The type folders of the picture modules.
pub const SHADER_CATEGORY: &str = "shaders";
pub const TRANSITION_CATEGORY: &str = "video_transitions";
pub const SHAPE_CATEGORY: &str = "shapes";
/// What a shape module draws.
pub const SHAPE_KINDS: [&str; 3] = ["square", "circle", "text"];
/// The four knobs every shader gets, numbered and 0..1.
pub const SHADER_PARAMS: [&str; 4] = ["P1", "P2", "P3", "P4"];
/// The kinds a bare `fade_in` means, and a fade naming a `chain` and no `with` means.
pub const DISSOLVE: &str = "dissolve";
pub const OVER: &str = "over";
/// The picture lane's strip, shipped in `meta/`, and the shader name the compiled scene carries of it.
pub const PICTURE_STRIP: &str = "@meta/shaders/video-mixer";
pub const PICTURE_STRIP_SHADER: &str = "video-mixer";

/// The plugin types that name no engine plugin: a pass on disk first, and a plugin k7 knows it cannot load.
pub const OFFLINE_TYPE: &str = "offline";

/// The range both engine levels take, in dB.
pub const VOLUME_RANGE: (f64, f64, f64) = (-60.0, 12.0, 0.0);
/// A rack's first pad note: C1, General MIDI's kick.
pub const RACK_FIRST_NOTE: u32 = 36;

/// The word for the project's own timeline: `timeline lane copy`'s destination.
pub const SONG_TARGET: &str = "main";
