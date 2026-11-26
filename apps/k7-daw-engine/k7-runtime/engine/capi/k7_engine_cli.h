/* k7_engine_cli.h — the cli area of the engine's C ABI: the one-shot modes of `k7-runtime` (scan, dumps, devices,
 * render, the multi-edit probe, decode), each the process's whole run, its exit code returned (docs/cliModes.md).
 * What a reader gets wrong: Rust parses argv and checks a document before it gets here, so a `doc_json` is the
 * validated document DocReader reads, with no check of its own; none of these runs beside a served host. */

#ifndef K7_ENGINE_CLI_H
#define K7_ENGINE_CLI_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

struct K7CliApi {
    /* The process's main thread, before any host; each returns the exit code. */
    int32_t (*scan_plugin_child)(K7Str format, K7Str id);
    int32_t (*scan_plugins)(int32_t force);
    int32_t (*dump_params)(K7Str type);
    int32_t (*dump_params_ext)(K7Str format, K7Str id, K7Str state_file);
    int32_t (*dump_builtin_params)(void);
    int32_t (*dump_surge_layout)(void);
    int32_t (*list_audio_devices)(void);
    /* `docs` is `count` validated documents. */
    int32_t (*multi_edit_test)(const K7Str* docs, size_t count, double seconds, K7Str device, K7Str device_type);
    int32_t (*render_offline)(K7Str doc_json, K7Str base_dir, K7Str out_dir, int32_t stems);
    int32_t (*decode)(K7Str path);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_CLI_H */
