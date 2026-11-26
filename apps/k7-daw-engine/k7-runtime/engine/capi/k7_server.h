/* k7_server.h — the Rust server's C door (apps/k7-daw-backend/packages/k7-runtime-server, linked as a static library):
 * what the engine's executable, Android's JNI and iOS's embed call to serve. Plain C, JUCE-free.
 * What a reader gets wrong: serve must run on the thread that becomes the host's message thread (Android's Looper
 * thread, iOS's serving thread) and blocks there until a quit; the host is created and destroyed inside it. */

#ifndef K7_SERVER_H
#define K7_SERVER_H

#ifdef __cplusplus
extern "C" {
#endif

/* Serves on `port` until k7_server_request_quit(); the process's exit code (0 on a quit). */
int k7_server_serve(int port);

/* Any thread: makes a running k7_server_serve return; a no-op when none runs. */
void k7_server_request_quit(void);

#ifdef __cplusplus
}
#endif

#endif /* K7_SERVER_H */
