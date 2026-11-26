/* k7_engine_core.h — the core area of the engine's C ABI: the process's setup, the host's life, its message
 * thread, and the few answers any thread may read. The server's first calls, and the ones every area relies on.
 * What a reader gets wrong: host_create must run on the thread that will call run(), since the first JUCE call
 * makes that thread the message thread (Android's Looper, iOS's run loop): apps/k7-daw-backend/packages/k7-runtime-server/docs/abi.md. */

#ifndef K7_ENGINE_CORE_H
#define K7_ENGINE_CORE_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

/* The interface's timing as the message thread last published it: zero before a device opened. */
typedef struct K7AudioTiming {
    int32_t buffer_frames;
    double sample_rate;
} K7AudioTiming;

/* One engine warning: `session` empty when it belongs to no project. */
typedef struct K7Warning {
    K7Str session;
    K7Str subject;
    K7Str message;
} K7Warning;

typedef void (*K7WarningSink)(void* ctx, const K7Warning* warning);

struct K7CoreApi {
    /* The process, before any host: any thread, in this order. */
    /* 0 when ProjectDoc has another layout in another object: a half-rebuilt engine. */
    int32_t (*layout_consistent)(void);
    void (*install_crash_handler)(void);
    /* The --settings-dir value, empty for none; $K7_SETTINGS_DIR is read as well (docs/settingsDir.md). */
    void (*init_settings_dir)(K7Str flag);
    void (*settings_dir)(void* ctx, K7StrEach out);
    void (*state_path)(void* ctx, K7StrEach out);
    void (*enable_load_guards)(void);
    void (*set_safe_mode)(int32_t on);
    /* The state folder and $STUDIO_ENGINE_ROOTS, plus temp for reads (docs/pathRoots.md). */
    void (*use_default_path_roots)(void);
    /* Set before the server accepts and cleared, NULL, after it stops (Warnings.h); the sink is called from any thread. */
    void (*set_warning_sink)(K7WarningSink sink, void* ctx);

    /* The host: created, run and destroyed on one thread. */
    /* NULL when the engine could not be built, the reason on stderr. */
    K7Host* (*host_create)(void);
    void (*host_destroy)(K7Host* host);
    /* Blocks on the message loop until quit(). */
    void (*run)(K7Host* host);
    /* Any thread. */
    void (*quit)(K7Host* host);
    /* Any thread: `run` later on the message thread, or `drop` once if it never will. */
    void (*post)(K7Host* host, K7Task run, K7Task drop, void* ctx);

    /* The message thread only. */
    void (*open_audio_device)(K7Host* host);
    void (*output_device_name)(K7Host* host, void* ctx, K7StrEach out);
    void (*suspend_audio)(K7Host* host, int32_t suspended);

    /* Any thread: atomics and the session registry's own lock. */
    K7AudioTiming (*audio_timing)(const K7Host* host);
    double (*message_thread_age_ms)(const K7Host* host);
    void (*session_ids)(const K7Host* host, void* ctx, K7StrEach each);
    int32_t (*has_session)(const K7Host* host, K7Str id);

    /* The process, before any host: a throw nothing caught is said on stderr before the abort. */
    void (*install_terminate_handler)(void);
    /* Any thread, after enable_load_guards: the file the last engine died loading, and the plugin it loaded last. */
    void (*previous_load_death)(void* ctx, K7StrEach out);
    void (*previous_last_plugin)(void* ctx, K7StrEach out);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_CORE_H */
