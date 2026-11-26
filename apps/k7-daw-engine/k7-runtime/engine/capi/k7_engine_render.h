/* k7_engine_render.h — the render area of the engine's C ABI: a validated document bounced to wav inside the
 * running engine, which a machine that may start no program asks for over the socket.
 * What a reader gets wrong: `render` blocks its caller for the whole bounce and needs the message loop running, so
 * it is never called from the message thread; the engine bounces on a headless engine of its own beside the live one. */

#ifndef K7_ENGINE_RENDER_H
#define K7_ENGINE_RENDER_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

/* What one bounce came to: 0 and the manifest `--render` prints, or a non-zero code and why. */
typedef struct K7RenderOutcome {
    int32_t code;
    K7Str manifest;
    K7Str error;
} K7RenderOutcome;

typedef void (*K7RenderDone)(void* ctx, const K7RenderOutcome* outcome);

struct K7RenderApi {
    /* Any thread but the message thread. */
    /* Bounces the validated document into `out_dir` (stems too when `stems` is 1); `done` is called once. */
    void (*render)(K7Str doc_json, K7Str out_dir, int32_t stems, void* ctx, K7RenderDone done);
    /* Any thread. */
    /* Once the message loop has stopped: tells every render in progress to give up what it waited on. */
    void (*cancel)(void);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_RENDER_H */
