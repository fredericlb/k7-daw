/* k7_engine_project.h — the project area of the engine's C ABI: a validated document handed in as JSON, the
 * session's tracks replaced, the project forgotten, and the path roots the engine enforces.
 * What a reader gets wrong: the document is ALREADY checked (bounds, ids, paths) by the Rust server and read by
 * a reader with no checks of its own (engine/DocReader.h); the diff and the apply are this side's. */

#ifndef K7_ENGINE_PROJECT_H
#define K7_ENGINE_PROJECT_H

#include "k7_engine.h"
#include "k7_engine_core.h"

#ifdef __cplusplus
extern "C" {
#endif

struct K7ProjectApi {
    /* Any thread. */
    /* Whether a document may name `path` for reading (write 0) or writing (write 1): the roots serve() set. */
    int32_t (*path_allowed)(K7Str path, int32_t write);
    /* Says a warning as the engine's warn(): on stderr, and to the sink under `session` (empty for none). */
    void (*warn)(K7Str session, K7Str subject, K7Str message);
    /* The warnings `session`'s project was built with, one call each, oldest first. */
    void (*standing_warnings)(K7Str session, void* ctx, K7WarningSink each);

    /* The message thread only. */
    /* Drops the slot's chain when it is loaded; 1 when there was one. */
    int32_t (*release_slot)(K7Host* host);
    /* Applies the validated document; 1 and each change's description, or 0 and the reason through `error`. */
    int32_t (*apply)(K7Host* host, K7Str session, K7Str doc_json, void* ctx, K7StrEach change, K7StrEach error);
    /* The ids of the tracks the session holds, in document order; none when it holds no project. */
    void (*track_ids)(K7Host* host, K7Str session, void* ctx, K7StrEach each);
    /* Replaces tracks by position, `[{"index":n,"track":{...}}]` validated, then applies; 0 and the reason on failure. */
    int32_t (*apply_tracks)(K7Host* host, K7Str session, K7Str plan_json, void* ctx, K7StrEach error);
    /* Forgets the project: plugins, edit and diff baseline; the engine stays up. */
    void (*reset)(K7Host* host, K7Str session);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_PROJECT_H */
