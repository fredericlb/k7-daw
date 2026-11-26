/* k7_engine_transport.h — the transport area of the engine's C ABI: play, stop, seek and loop, the auditions,
 * a live keyboard, record arming and the recorder node. Every function runs on the message thread only.
 * What a reader gets wrong: a `want` of -1 means "toggle", resolved here rather than by the caller, so two
 * clients cannot flip the same light twice. */

#ifndef K7_ENGINE_TRANSPORT_H
#define K7_ENGINE_TRANSPORT_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

struct K7TransportApi {
    /* The message thread only. */
    void (*play)(K7Host* host, K7Str session);
    void (*stop)(K7Host* host, K7Str session);
    /* Stop, then tell every plugin to let go and reset. */
    void (*panic)(K7Host* host, K7Str session);
    void (*rewind)(K7Host* host, K7Str session);
    void (*seek)(K7Host* host, K7Str session, double seconds);
    void (*set_loop)(K7Host* host, K7Str session, double start_seconds, double end_seconds, int32_t enabled);
    int32_t (*is_playing)(K7Host* host, K7Str session);
    /* Audition a file from `start` for `length` seconds (<= 0 is up to 30 s), independent of the transport. */
    void (*preview_sample)(K7Host* host, K7Str session, K7Str file, double start, double length);
    void (*preview_note)(K7Host* host, K7Str session, K7Str track, int32_t key, float velocity);
    /* One channel message of a live keyboard into a track's own chain; `bytes` is borrowed for the call. */
    void (*live_midi)(K7Host* host, K7Str session, K7Str track, const uint8_t* bytes, size_t len);
    /* Arms or disarms (want 0 or 1, -1 toggles); 1 and the state in `armed`, or 0 and the reason through `error`. */
    int32_t (*set_armed)(K7Host* host, K7Str session, K7Str track, int32_t want, int32_t* armed, void* ctx, K7StrEach error);
    /* The recorder node's verbs: clear 1 pops the last take, else `on` starts or stops it (-1 toggles); 0 and the reason on refusal. */
    int32_t (*recorder)(K7Host* host, K7Str session, K7Str node, int32_t clear, int32_t on, void* ctx, K7StrEach error);
    /* Whether play() records the armed tracks, -1 toggles. */
    void (*set_record_enabled)(K7Host* host, K7Str session, int32_t want);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_TRANSPORT_H */
