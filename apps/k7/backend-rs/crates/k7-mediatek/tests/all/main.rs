// One test binary for the crate: every former tests/<name>.rs is the module <name>.
// Kept apart: analysis.rs, which sets FFPROBE_BIN for its whole process and would hide ffprobe from every other test.

mod engine_watchdog;
mod sidecar_atomic;
mod transcode;
mod video_copy;
mod video_probe;
mod video_read;
mod whisper;
