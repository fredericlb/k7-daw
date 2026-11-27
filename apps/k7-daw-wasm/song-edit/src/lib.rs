// lib.rs — the song editor in Rust, base: the workspace's k7x files, the module folders, confinement, chain item and lane strip writes.
// A reader would get wrong that nothing here touches a disk: every call goes through an `Fs` the host gives (docs/wasm-step3b-brief.md).

// The TypeScript's signatures are the contract, and a position is positional (beats, num, den).
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod account_fork;
pub mod alternative_file;
pub mod alternative_switch;
pub mod assembly_edit;
pub mod base_preset;
pub mod chain_create;
pub mod chain_data;
pub mod chain_graph_edit;
pub mod chain_item_edit;
pub mod chain_nest;
pub mod chain_project;
pub mod chain_segments;
pub mod chain_stack;
pub mod clip_edit;
pub mod clip_resize;
pub mod confine;
pub mod crossfade;
pub mod curve_file;
pub mod doc_names;
pub mod engine_switch;
pub mod error;
pub mod event_gain;
pub mod event_loop;
pub mod facade;
pub mod format_chain;
pub mod format_level;
pub mod format_stamp;
pub mod from_segments;
pub mod fs;
pub mod graph_edit;
pub mod graph_file;
pub mod graph_schema;
pub mod ids;
pub mod insert_time;
pub mod js_text;
pub mod lane_files;
pub mod lane_strip;
pub mod lane_strip_plan;
pub mod library_forks;
pub mod list_files;
pub mod loader;
pub mod material_cut;
pub mod material_file;
pub mod material_notes;
pub mod material_window;
pub mod mem_fs;
pub mod module_file;
pub mod module_fork;
pub mod module_io;
pub mod module_move;
pub mod node_files;
pub mod node_presets;
pub mod notes;
pub mod onsets;
pub mod open_repairs;
pub mod paths;
pub mod placement_move;
pub mod playlist;
pub mod preferences;
pub mod preferences_file;
pub mod presets;
pub mod project_edit;
pub mod project_root;
pub mod rack_taps;
pub mod picture_animate;
pub mod picture_file;
pub mod scaffold;
pub mod scaffold_files;
pub mod sequence_file;
pub mod sequence_loops;
pub mod shared_folders;
pub mod song_tempo;
pub mod start_time;
pub mod sugar;
pub mod take_edit;
pub mod take_file;
pub mod takes;
pub mod tap_chain;
pub mod tasks;
pub mod time_splice;
pub mod timeline_from_range;
pub mod timeline_rename;
pub mod track_edit;
pub mod track_file;
pub mod track_mix;
pub mod video_outputs;
pub mod winpath;
pub mod workspace_doc;
pub mod workspace_file;
pub mod yaml_edit;

pub use error::{Error, Res};
pub use fs::Fs;
pub use mem_fs::MemFs;

// 3j: the server's own edits, in one block so a merge with 3g2's additions is a clean one.
pub mod instance_name;
pub mod project_files;
pub mod record_take;
// 3j M5: the bench's rig.
pub mod scaffold_live;
pub mod scaffold_needs;
pub mod scaffold_set;
pub mod scaffold_set_files;
pub mod scaffold_set_nodes;
pub mod scaffold_set_sources;
