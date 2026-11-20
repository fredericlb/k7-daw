//! k7-sync — a workspace's history rooms and what the capture writes with: the ordered log on disk (`log`, `room`,
//! `relay`), the settle of each op through `k7-js`'s number formats (`settle`, `diff3`, `k7x`), the history reads and
//! restores (`history`), and the file, JSON and path helpers. Choices: `apps/k7/backend-rs/docs/files/history.md`.
//! What a reader gets wrong: an entry is on disk before the room's memory takes it, so a crash never leaves it ahead.

pub mod bases;
pub mod conflicts;
pub mod diff3;
pub mod fsx;
pub mod history;
pub mod json;
pub mod k7x;
pub mod log;
pub mod path;
pub mod protocol;
pub mod relay;
pub mod room;
pub mod settle;
