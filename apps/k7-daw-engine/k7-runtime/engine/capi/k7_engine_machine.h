/* k7_engine_machine.h — the machine area of the engine's C ABI: the sessions it holds and who drives them, the
 * audio interface and its inputs, the audio callback's cost, and the master tap the broadcast drains.
 * What a reader gets wrong: `drain_broadcast` and `skip_broadcast_to_live` belong to ONE thread (the tap's ring is
 * single-consumer), and the last group is the message thread's; the first group reads atomics or files. */

#ifndef K7_ENGINE_MACHINE_H
#define K7_ENGINE_MACHINE_H

#include "k7_engine.h"

#ifdef __cplusplus
extern "C" {
#endif

/* One session as `list-sessions` reports it; `project` is meaningless unless `loaded`. */
typedef struct K7SessionInfo {
    K7Str id;
    K7Str project;
    int32_t loaded;
    int32_t playing;
    int32_t track_count;
} K7SessionInfo;

/* One audio interface a backend offers; `current` is its output side, `current_input` its input side. */
typedef struct K7AudioDevice {
    K7Str name;
    K7Str type;
    int32_t inputs;
    int32_t outputs;
    int32_t current;
    int32_t current_input;
    int32_t is_default;
} K7AudioDevice;

/* A channel set: the 1-based channels one meter listens to. */
typedef struct K7IntList {
    const int32_t* ptr;
    size_t len;
} K7IntList;

/* One channel set's peak in dB (-100 silent); `found` is 0 when the rig has no device carrying it. */
typedef struct K7ChannelLevel {
    float db;
    int32_t found;
} K7ChannelLevel;

/* One channel set's pitch; `hz` is 0 when nothing convincing was heard. */
typedef struct K7ChannelPitch {
    float hz;
    float clarity;
} K7ChannelPitch;

/* One wave input device with the 1-based channels it carries. */
typedef struct K7InputDevice {
    K7Str name;
    const int32_t* channels;
    size_t channel_count;
    int32_t enabled;
} K7InputDevice;

/* What the audio callback has cost since the last reset, in seconds. */
typedef struct K7CallbackCost {
    double mean_seconds;
    double max_seconds;
    double min_seconds;
    int64_t runs;
    double deadline_seconds;
} K7CallbackCost;

/* What a drain answered: `lost_frames` is what the writer overwrote before the sender reached it. */
typedef struct K7BroadcastInfo {
    size_t frames;
    size_t lost_frames;
    double sample_rate;
} K7BroadcastInfo;

typedef void (*K7SessionEach)(void* ctx, const K7SessionInfo* info);
typedef void (*K7AudioDeviceEach)(void* ctx, const K7AudioDevice* device);
typedef void (*K7ChannelLevelEach)(void* ctx, const K7ChannelLevel* level);
typedef void (*K7ChannelPitchEach)(void* ctx, const K7ChannelPitch* pitch);
typedef void (*K7InputDeviceEach)(void* ctx, const K7InputDevice* device);

struct K7MachineApi {
    /* Any thread. */
    K7CallbackCost (*callback_cost)(K7Host* host);
    void (*reset_callback_cost)(K7Host* host);
    /* 1 when `ref` (an `@instruments/` one or a path) can load on this machine: file stats only, no host. */
    int32_t (*instrument_installed)(K7Str ref);

    /* One thread, the broadcast sender's. */
    /* Takes up to `max_frames` stereo frames of the session's master tap into `samples` (2 * max_frames floats). */
    void (*drain_broadcast)(K7Host* host, K7Str session, size_t max_frames, float* samples, K7BroadcastInfo* info);
    /* Drops what is buffered so a listener who just arrived hears now. */
    void (*skip_broadcast_to_live)(K7Host* host, K7Str session);

    /* The message thread only. */
    /* A socket driving `session` arrived or went: a project nobody drives is released after a grace. */
    void (*add_driver)(K7Host* host, K7Str session);
    void (*remove_driver)(K7Host* host, K7Str session);
    /* The tap on the master plugin list on or off; ON creates the session, OFF never does. */
    void (*enable_broadcast)(K7Host* host, K7Str session, int32_t on);
    /* Keeps the session's mix from the interface while still broadcasting it; OFF never creates the session. */
    void (*silence_output)(K7Host* host, K7Str session, int32_t on);
    /* Every session in id order, one call each. */
    void (*session_infos)(K7Host* host, void* ctx, K7SessionEach each);
    void (*drop_session)(K7Host* host, K7Str session);
    /* Every interface, ASIO first, one call each. */
    void (*list_audio_devices)(K7Host* host, void* ctx, K7AudioDeviceEach each);
    /* Opens one for input and output; 0 when none by that name and type (an empty type is any backend). */
    int32_t (*set_audio_device)(K7Host* host, K7Str name, K7Str type);
    /* One reading per channel set, in order: the interface's own, off the tuner's tap, which the first ask attaches. */
    void (*channel_levels_db)(K7Host* host, const K7IntList* sets, size_t set_count, void* ctx, K7ChannelLevelEach each);
    void (*channel_pitches)(K7Host* host, const K7IntList* sets, size_t set_count, void* ctx, K7ChannelPitchEach each);
    void (*input_devices)(K7Host* host, void* ctx, K7InputDeviceEach each);
    /* Pairs the input carrying `channel` (1-based) with its partner or splits it; 0 with no interface open. */
    int32_t (*set_input_stereo)(K7Host* host, int32_t channel, int32_t stereo);
    /* Peak dB leaving the session's master, -100 with no edit loaded. */
    float (*master_level_db)(K7Host* host, K7Str session);
};

#ifdef __cplusplus
}
#endif

#endif /* K7_ENGINE_MACHINE_H */
