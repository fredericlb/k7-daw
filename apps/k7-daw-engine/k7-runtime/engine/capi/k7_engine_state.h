/* k7_engine_state.h — the state area of the engine's C ABI: the state frame the engine's timer produces about
 * 20 times a second per session, the takes and parameter events it pushes, and the few calls on them.
 * What a reader gets wrong: every pointer in a frame or an event is borrowed for the call that carries it, so the
 * server copies what it keeps; the sinks run on the message thread, so they must not block (abi.md beside the Rust crate). */

#ifndef K7_ENGINE_STATE_H
#define K7_ENGINE_STATE_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

/* One bucket of a waveform or a recorder picture, -1..1. */
typedef struct K7Envelope {
    float min;
    float max;
} K7Envelope;

/* A finished take of a recorder node. */
typedef struct K7TakeInfo {
    K7Str file;
    double seconds;
    int32_t damaged;
} K7TakeInfo;

/* A module's waveform buckets since the last frame, keyed "chain:module:side". */
typedef struct K7WaveformEntry {
    K7Str key;
    const K7Envelope* points;
    size_t count;
} K7WaveformEntry;

/* A platter's needle, in seconds of its own record. */
typedef struct K7PlatterEntry {
    K7Str id;
    double seconds;
} K7PlatterEntry;

/* A recorder node: its clock, its takes and its picture since the last frame, which starts at `envelope_from`. */
typedef struct K7RecorderEntry {
    K7Str id;
    int32_t recording;
    double seconds;
    const K7TakeInfo* takes;
    size_t take_count;
    const K7Envelope* envelope;
    size_t envelope_count;
    uint64_t envelope_from;
} K7RecorderEntry;

/* A plugin's cost; `bytes` is -1 when it cannot say. */
typedef struct K7LoadEntry {
    K7Str id;
    double cpu;
    int64_t bytes;
} K7LoadEntry;

/* One session's snapshot, the engine's own state timer's; empty `session` is the default one. */
typedef struct K7StateFrame {
    K7Str session;
    int32_t playing;
    double playhead;
    int32_t loop_on;
    double loop_start;
    double loop_end;
    const float* levels;
    size_t level_count;
    const float* input_levels;
    size_t input_level_count;
    const K7Str* armed;
    size_t armed_count;
    int32_t record_enabled;
    float master_db;
    int32_t master_clipped;
    const K7WaveformEntry* waveforms;
    size_t waveform_count;
    const K7PlatterEntry* platters;
    size_t platter_count;
    const K7RecorderEntry* recorders;
    size_t recorder_count;
    /* Nonzero on the frames that carry the cost report; `process_bytes` is -1 when unknown. */
    int32_t has_load;
    const K7LoadEntry* loads;
    size_t load_count;
    int64_t process_bytes;
} K7StateFrame;

/* One parameter of a watched plugin with its value. */
typedef struct K7ParamValue {
    K7Str name;
    double value;
} K7ParamValue;

/* A discrete parameter's named setting and the value that selects it. */
typedef struct K7ParamState {
    K7Str label;
    double value;
} K7ParamState;

/* A parameter with what a control needs to draw itself, in the parameter's own units. */
typedef struct K7ParamInfo {
    K7Str name;
    double value;
    double min;
    double max;
    int32_t has_default;
    double def;
    const K7ParamState* states;
    size_t state_count;
} K7ParamInfo;

typedef void (*K7StateSink)(void* ctx, const K7StateFrame* frame);
/* A take finished, closed and named: start and length in seconds. */
typedef void (*K7RecordedSink)(void* ctx, K7Str session, K7Str track, K7Str file, double start, double length);
/* A plugin's editor closed and its state was captured, as the JSON text the engine built. */
typedef void (*K7ParamsSink)(void* ctx, K7Str session, K7Str id, K7Str params_json);
/* A watched plugin's full parameter set, whenever a value moved. */
typedef void (*K7ParamsLiveSink)(void* ctx, K7Str session, K7Str id, const K7ParamValue* params, size_t count);
typedef void (*K7ParamInfoEach)(void* ctx, const K7ParamInfo* info);

/* The four sinks, one context: set once per run, and never freed. */
typedef struct K7StateSinks {
    void* ctx;
    K7StateSink state;
    K7RecordedSink recorded;
    K7ParamsSink params;
    K7ParamsLiveSink params_live;
} K7StateSinks;

struct K7StateApi {
    /* The message thread only. */
    /* Starts the state timer at `interval_ms` and installs the sinks, which are then called on this thread. */
    void (*start)(K7Host* host, const K7StateSinks* sinks, int32_t interval_ms);
    /* A new subscriber holds none of the take being recorded: the next frame carries the whole picture. */
    void (*rewind_recorder_envelopes)(K7Host* host, K7Str session);
    /* On or off, for a plugin of this session; an id not loaded is accepted. */
    void (*watch_plugin_params)(K7Host* host, K7Str session, K7Str id, int32_t on);
    /* Every automatable parameter of a live plugin with its current value, none when there is no such plugin. */
    void (*plugin_param_infos)(K7Host* host, K7Str session, K7Str id, void* ctx, K7ParamInfoEach each);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_STATE_H */
