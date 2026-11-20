// doc/mod.rs — the ProjectDoc and the passes that make it (sub-step 2h, package B); nothing here is wired to a route.
// A reader would get wrong that the types are the whole module: each milestone adds the file it names in the work order.

mod arrange;
mod automation;
mod build;
mod category;
mod compile;
mod finish;
mod freeze_points;
mod highlight;
mod lane_tracks;
mod lanes;
mod notes;
mod own_mix;
mod plugin;
mod plugins;
mod recorder;
mod texts;
mod types;
mod wire;

pub use arrange::{ParsedName, TrackFamily, order_tracks, parse_track_name, sort_tracks_by_kind, track_kind_rank};
pub use category::{TrackCategory, auto_colors, category_of_text, category_shade, track_category};
pub use compile::{Compiled, Failure, Request, Spent, Vsts, compile, compile_keeping};
pub use freeze_points::{FROZEN_CLIP_SUFFIX, frozen_clip};
pub use highlight::highlighted;
pub use lanes::Pictures;
pub use own_mix::{
    BlendShape, INPUT_VOLUME_PARAM, LevelSide, MIX_PARAM, MixSide, OUTPUT_VOLUME_PARAM, held_params, own_level_side,
    own_mix_side, owns_blend, takes_input_volume, takes_output_volume,
};
pub use recorder::{OUTPUT_RECORDER_ID, RECORDER_PLUGIN, recorder_plugin, with_output_recorder};
pub use texts::jsdsp_finding;
pub use types::{
    AutomationDoc, GroupDoc, InputFromDoc, PluginDoc, ProjectDoc, RackSoundDoc, SendDoc, TapOutDoc, TrackDoc,
    text_weight,
};
pub use wire::with_group_mix;

/// What one pass may COPY, weighed as `PluginDoc::weight` and `TrackDoc::weight` weigh it (decision 323).
pub const BUDGET: usize = 4_000_000;
