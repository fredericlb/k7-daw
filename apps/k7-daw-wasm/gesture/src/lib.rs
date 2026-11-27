// lib.rs — the arithmetic of the page's gestures: what a drag lands on, what a window shows, what a tool writes, where a ruler ticks.
// A reader would get wrong that every function here answers what JavaScript's would, down to `Math.round`'s half-up and `toFixed`'s ties (js.rs).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod bar_window;
pub mod clip_playhead;
pub mod clip_warp;
pub mod edge_drag;
pub mod format_at;
pub mod grid_caret;
pub mod js;
pub mod layout;
pub mod pan_zoom;
pub mod roll_drag;
pub mod roll_fit;
pub mod ruler;
pub mod snap;
pub mod take_counter;
pub mod ticks;
pub mod tools;
pub mod video_drag;
