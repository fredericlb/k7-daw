/* k7_server.h — the Rust server's C doors (apps/k7-daw-backend/packages/k7-runtime-server, linked as a static
 * library): what the engine's executable, Android's JNI and iOS's embed call. Plain C, JUCE-free.
 * What a reader gets wrong: serve must run on the thread that becomes the host's message thread (the process's main
 * thread, Android's Looper thread, iOS's serving thread) and blocks there until a quit; the host lives inside it. */

#ifndef K7_SERVER_H
#define K7_SERVER_H

#ifdef __cplusplus
extern "C" {
#endif

/* The executable's whole run from C's argv: a one-shot mode, or the server on the port argv names. */
int k7_server_main(int argc, const char* const* argv);

/* Serves on `port` until k7_server_request_quit(); the process's exit code (0 on a quit). */
int k7_server_serve(int port);

/* Any thread: makes a running k7_server_serve return; a no-op when none runs. */
void k7_server_request_quit(void);

/* Any thread: closes the audio device (non-zero) or reopens it (zero); a no-op when none runs. */
void k7_server_suspend_audio(int suspended);

/* Before serving: the server's own stderr lines start with `[engine] ` (the iOS app, whose stderr the front shares). */
void k7_server_tag_log(void);

#ifdef __cplusplus
}
#endif

#endif /* K7_SERVER_H */
