// reads/mod.rs — what the page reads off the open project's document: takes, loops, spans, rows, the graph's inputs and racks.
// A reader would get wrong that these take a SLIM copy of the page's WebProject: a field no rule reads is not declared here.
// Every answer is plain data; the page makes of it the objects it holds (docs/reads.md).

mod activity;
mod blank;
mod clip;
mod graph;
mod misc;
mod model;
mod rack;
mod rows;
mod shell;
mod spans;
mod takes;

pub use activity::{activity_mask, audio_activity, midi_activity, to_alphas};
pub use blank::{Blanks, blanks};
pub use clip::{ClipLoop, Instances, clip_loop, material_instances, mixer_modules, played_zone};
pub use graph::{
    LaneInput, PlayedTarget, gives_channel, has_audible_input, lane_input, midi_in_device_for, midi_in_lanes,
    midi_in_played, rack_for, rack_key_names, without_notes,
};
pub use misc::{
    ClipNameParts, FIX_TARGET_DB, FolderName, VOLUME_DB_MAX, VOLUME_DB_MIN, clip_name_parts, fixed_master, folder_name, follow_rename, shape_of_surfaces, tap_tempo,
    trim_for, TapRun,
};
pub use model::{RChain, RClip, RGraph, RGroup, RNote, RRack, RStep, RTrack, RVideoClip, RVideoTrack};
pub use rack::{Hit, rack_hits};
pub use rows::{LayoutRow, placement_of, project_rows, track_placing};
pub use shell::{DocTrack, LaneMix, RecInfo, SeqRef, ShellView, lane_mix, shell_view};
pub use spans::{LaneNotes, LaneSpan, NoteSpan, note_spans, video_spans};
pub use takes::{
    CompBeats, Crossfade, TakeRow, TakesEvent, arm_line, comp_fade_line, comp_line, comp_seams, heard_spans,
    piece_edge_lines, takes_event_of,
};
