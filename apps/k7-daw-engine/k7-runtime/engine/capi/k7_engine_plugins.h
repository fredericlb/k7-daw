/* k7_engine_plugins.h — the plugins area of the engine's C ABI: the installed catalogue and its scan, a plugin
 * type's parameters, the live instances of a project, their windows, presets and waveform taps.
 * What a reader gets wrong: the functions marked BLOCKING load plugin binaries, take seconds to minutes, and
 * must never run on the message thread (they hand work to it themselves); the rest are message thread only. */

#ifndef K7_ENGINE_PLUGINS_H
#define K7_ENGINE_PLUGINS_H

#include "k7_engine.h"
#include "k7_engine_state.h"

#ifdef __cplusplus
extern "C" {
#endif

/* One installed plugin of the catalogue, as `plugins` lists it. */
typedef struct K7PluginInfo {
    K7Str name;
    K7Str format;
    K7Str manufacturer;
    int32_t instrument;
    K7Str role;
    K7Str category;
    K7Str id;
} K7PluginInfo;

/* One plugin instance of a loaded project. */
typedef struct K7PluginInstance {
    K7Str id;
    K7Str name;
    int32_t has_native_ui;
} K7PluginInstance;

/* A selectable preset: `source` is "program" (by `index`) or "file" (by `path`). */
typedef struct K7Preset {
    K7Str name;
    K7Str source;
    int32_t index;
    K7Str path;
} K7Preset;

typedef void (*K7PluginEach)(void* ctx, const K7PluginInfo* info);
typedef void (*K7InstanceEach)(void* ctx, const K7PluginInstance* instance);
typedef void (*K7PresetEach)(void* ctx, const K7Preset* preset);
/* Where a scan stands: files done of those listed (0 while listing), and the file being loaded, empty for one only stat'ed. */
typedef void (*K7ScanStep)(void* ctx, int32_t done, int32_t total, K7Str loading);

struct K7PluginsApi {
    /* Any thread. */
    /* Nonzero when the catalogue's cache needs no scan: written, holding this binary's built-ins, and an external if this build can host one. */
    int32_t (*cache_warm)(void);
    /* The cached catalogue, built-ins first, no scanning. */
    void (*load_cache)(void* ctx, K7PluginEach each);
    /* BLOCKING. Scans (incrementally, or from nothing when `force`), calling `step` as it goes and `each` with the catalogue it ended on; 0 and `failed` told why when it threw. */
    int32_t (*scan)(int32_t force, void* ctx, K7ScanStep step, K7PluginEach each, K7StrEach failed);
    /* BLOCKING. Every parameter of an installed plugin TYPE, `value` always 0; 0 means it could not be found, created or loaded, and 1 with none handed means it publishes none. */
    int32_t (*plugin_params_of)(K7Str format, K7Str id, void* ctx, K7ParamInfoEach each);
    /* BLOCKING. Loads the plugin's orphan instance for its own window; 0 when it did not load. */
    int32_t (*prepare_native_ui)(K7Str format, K7Str id);

    /* The message thread only. */
    /* Opens the window of a prepared orphan, 1 when it opened. */
    int32_t (*show_native_ui_of)(K7Host* host, K7Str format, K7Str id);
    /* Writes what the orphan holds to the absolute `path` as a .vstpreset, 1 when it wrote. */
    int32_t (*snapshot_orphan_to)(K7Host* host, K7Str format, K7Str id, K7Str path);
    /* Opens a live plugin's own window; external plugins only. */
    void (*show_native_ui)(K7Host* host, K7Str session, K7Str id);
    void (*plugin_instances)(K7Host* host, K7Str session, void* ctx, K7InstanceEach each);
    void (*list_presets)(K7Host* host, K7Str session, K7Str module, void* ctx, K7PresetEach each);
    /* A non-empty `path` wins over `index`. */
    void (*set_preset)(K7Host* host, K7Str session, K7Str module, int32_t index, K7Str path);
    /* One automatable parameter of the live plugin, in its native range; never persisted. */
    void (*set_plugin_param)(K7Host* host, K7Str session, K7Str id, K7Str name, double value);
    void (*enable_waveform_tap)(K7Host* host, K7Str session, K7Str id, K7Str side);
    void (*disable_waveform_tap)(K7Host* host, K7Str session, K7Str id, K7Str side);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_PLUGINS_H */
