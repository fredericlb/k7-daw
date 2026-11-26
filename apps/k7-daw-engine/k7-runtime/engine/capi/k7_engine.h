/* k7_engine.h — the engine's C ABI: the one door the Rust server (apps/k7-daw-backend/packages/k7-runtime-server)
 * calls the Tracktion host through. One exported symbol, k7_engine_api(), hands a table of area tables, each
 * area's functions in a header of its own; an area not built yet is NULL. Plain C, JUCE-free.
 * What a reader gets wrong: nothing crosses with its ownership. A K7Str or a struct is borrowed for the
 * duration of the call that hands it over, and the receiver copies what it keeps. The threading contract
 * (which functions run on the message thread only) is per function: apps/k7-daw-backend/packages/k7-runtime-server/docs/abi.md. */

#ifndef K7_ENGINE_H
#define K7_ENGINE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The table's version: raised when a function changes its signature or meaning, never for one appended. */
#define K7_ENGINE_ABI 1

/* UTF-8 bytes, not NUL-terminated, borrowed for the call. */
typedef struct K7Str {
    const char* ptr;
    size_t len;
} K7Str;

/* The engine host: one per process, created and run on the thread that becomes its message thread. */
typedef struct K7Host K7Host;

/* A job and its context: `run` on the message thread, or `drop` instead when it can no longer run. */
typedef void (*K7Task)(void* ctx);

/* Hands each string of a list, or the one string of an answer, to its caller's context. */
typedef void (*K7StrEach)(void* ctx, K7Str item);

/* The area tables, each defined in its own header by the milestone that builds it. */
typedef struct K7CoreApi K7CoreApi;
typedef struct K7ProjectApi K7ProjectApi;
typedef struct K7TransportApi K7TransportApi;
typedef struct K7StateApi K7StateApi;
typedef struct K7PluginsApi K7PluginsApi;
typedef struct K7MachineApi K7MachineApi;
typedef struct K7RenderApi K7RenderApi;
typedef struct K7CliApi K7CliApi;

typedef struct K7EngineApi {
    uint32_t abi;
    /* sizeof(K7EngineApi) as this engine was built, so a caller refuses a table shorter than its own. */
    uint32_t size;
    const K7CoreApi* core;
    const K7ProjectApi* project;
    const K7TransportApi* transport;
    const K7StateApi* state;
    const K7PluginsApi* plugins;
    const K7MachineApi* machine;
    const K7RenderApi* render;
    const K7CliApi* cli;
} K7EngineApi;

/* The table, built once, valid for the life of the process. */
const K7EngineApi* k7_engine_api(void);

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_H */
