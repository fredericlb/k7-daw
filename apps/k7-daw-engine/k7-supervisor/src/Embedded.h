// Embedded.h — the supervisor as a library, for the iOS app, which may run no program of its own: the
// C entry its Swift half calls. What the app writes to `commandsFd` and reads from `framesFd` is the
// NDJSON a desktop app exchanges with the program over stdio (apps/k7-daw-engine/k7-runtime/docs/ios.md).

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/** Run until the app closes the write end of `commandsFd` (its quit) or the supervisor exits; blocks the calling thread. */
int k7_supervisor_run(int commandsFd, int framesFd);

/** Stem cuts queued or running in this process: the app stays alive behind another for as long as it is above zero. */
int k7_supervisor_cuts_in_flight(void);

/** Append one `[tag] text` line to the machine log, as a child's line is appended; any thread, any time. */
void k7_supervisor_log(const char* tag, const char* text);

#ifdef __cplusplus
}
#endif
