// pidkill.rs — the last-resort cleanup on quit, for the case where the supervisor never got there (hung, or already
// crashed): an engine that survives the app holds the audio device and both ports. Only a process the pidfile PROVES
// is that engine is killed; the proof is `k7-local`'s, which the front's supervisor host uses too. See docs/tray.md.

use crate::paths::engine_pid_path;

/// Kill the engine the pidfile names, once proved to be it; the file stays while its process may live (docs/tray.md).
pub fn backstop() {
    k7_local::pidfile::backstop(&engine_pid_path(), &|text| {
        crate::logbus::line("host", text)
    });
}
