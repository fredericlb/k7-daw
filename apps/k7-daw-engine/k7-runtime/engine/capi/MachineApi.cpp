// MachineApi.cpp — the machine area of the C ABI (k7_engine_machine.h) over EngineHost: one thin call per verb.
// What a reader gets wrong: a list is handed one entry per callback with pointers into this function's stack, so the
// server copies what it keeps; nothing here posts, the message-thread functions are called on that thread.

#include "k7_engine_machine.h"

#include "CApi.h"

#include "../EngineHost.h"
#include "../InstrumentStatus.h"

#include <algorithm>
#include <string>
#include <vector>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }

std::vector<std::vector<int>> sets(const K7IntList* in, size_t count) {
    std::vector<std::vector<int>> out;
    if (in == nullptr) return out;
    out.reserve(count);
    for (size_t i = 0; i < count; ++i) out.emplace_back(in[i].ptr, in[i].ptr + in[i].len);
    return out;
}

K7CallbackCost callbackCost(K7Host* h) {
    return guard<K7CallbackCost>("callback_cost", K7CallbackCost{}, [&] {
        const auto c = host(h)->callbackCost();
        return K7CallbackCost{c.meanSeconds, c.maxSeconds, c.minSeconds, c.runs, c.deadlineSeconds};
    });
}

void resetCallbackCost(K7Host* h) { guardVoid("reset_callback_cost", [&] { host(h)->resetCallbackCost(); }); }

int32_t instrumentInstalledC(K7Str ref) {
    return guard<int32_t>("instrument_installed", 0, [&] { return instrumentInstalled(str(ref)) ? 1 : 0; });
}

void drainBroadcast(K7Host* h, K7Str s, size_t maxFrames, float* samples, K7BroadcastInfo* info) {
    guardVoid("drain_broadcast", [&] {
        if (info != nullptr) *info = K7BroadcastInfo{};
        const auto chunk = host(h)->drainBroadcast(str(s), maxFrames);
        const size_t frames = std::min(chunk.frames, maxFrames);
        const size_t count = frames * static_cast<size_t>(EngineHost::broadcastChannels());
        if (samples != nullptr) std::copy_n(chunk.samples.begin(), std::min(count, chunk.samples.size()), samples);
        if (info != nullptr) *info = K7BroadcastInfo{frames, chunk.lostFrames, chunk.sampleRate};
    });
}

void skipBroadcastToLive(K7Host* h, K7Str s) {
    guardVoid("skip_broadcast_to_live", [&] { host(h)->skipBroadcastToLive(str(s)); });
}

void addDriver(K7Host* h, K7Str s) { guardVoid("add_driver", [&] { host(h)->addDriver(str(s)); }); }
void removeDriver(K7Host* h, K7Str s) { guardVoid("remove_driver", [&] { host(h)->removeDriver(str(s)); }); }

void enableBroadcast(K7Host* h, K7Str s, int32_t on) {
    guardVoid("enable_broadcast", [&] { host(h)->enableBroadcast(str(s), on != 0); });
}

void silenceOutput(K7Host* h, K7Str s, int32_t on) {
    guardVoid("silence_output", [&] { host(h)->silenceOutput(str(s), on != 0); });
}

void sessionInfos(K7Host* h, void* ctx, K7SessionEach each) {
    guardVoid("session_infos", [&] {
        if (each == nullptr) return;
        for (const auto& s : host(h)->sessionInfos()) {
            const K7SessionInfo info{view(s.id), view(s.project), s.loaded ? 1 : 0, s.playing ? 1 : 0, s.trackCount};
            each(ctx, &info);
        }
    });
}

void dropSession(K7Host* h, K7Str s) { guardVoid("drop_session", [&] { host(h)->dropSession(str(s)); }); }

void listAudioDevices(K7Host* h, void* ctx, K7AudioDeviceEach each) {
    guardVoid("list_audio_devices", [&] {
        if (each == nullptr) return;
        for (const auto& d : host(h)->listAudioDevices()) {
            const K7AudioDevice device{view(d.name),       view(d.type),          d.inputs,
                                       d.outputs,          d.current ? 1 : 0,     d.currentInput ? 1 : 0,
                                       d.isDefault ? 1 : 0};
            each(ctx, &device);
        }
    });
}

int32_t setAudioDevice(K7Host* h, K7Str name, K7Str type) {
    return guard<int32_t>("set_audio_device", 0, [&] { return host(h)->setAudioDevice(str(name), str(type)) ? 1 : 0; });
}

void channelLevelsDb(K7Host* h, const K7IntList* in, size_t count, void* ctx, K7ChannelLevelEach each) {
    guardVoid("channel_levels_db", [&] {
        if (each == nullptr) return;
        for (const auto& l : host(h)->channelLevelsDb(sets(in, count))) {
            const K7ChannelLevel level{l.db, l.found ? 1 : 0};
            each(ctx, &level);
        }
    });
}

void channelPitches(K7Host* h, const K7IntList* in, size_t count, void* ctx, K7ChannelPitchEach each) {
    guardVoid("channel_pitches", [&] {
        if (each == nullptr) return;
        for (const auto& p : host(h)->channelPitches(sets(in, count))) {
            const K7ChannelPitch pitch{p.hz, p.clarity};
            each(ctx, &pitch);
        }
    });
}

void inputDevices(K7Host* h, void* ctx, K7InputDeviceEach each) {
    guardVoid("input_devices", [&] {
        if (each == nullptr) return;
        for (const auto& d : host(h)->inputDevices()) {
            const std::vector<int32_t> channels(d.channels.begin(), d.channels.end());
            const K7InputDevice device{view(d.name), channels.data(), channels.size(), d.enabled ? 1 : 0};
            each(ctx, &device);
        }
    });
}

int32_t setInputStereo(K7Host* h, int32_t channel, int32_t stereo) {
    return guard<int32_t>("set_input_stereo", 0, [&] { return host(h)->setInputStereo(channel, stereo != 0) ? 1 : 0; });
}

float masterLevelDb(K7Host* h, K7Str s) {
    return guard<float>("master_level_db", -100.0f, [&] { return host(h)->masterLevelDb(str(s)); });
}

const K7MachineApi kMachine{
    callbackCost, resetCallbackCost, instrumentInstalledC, drainBroadcast, skipBroadcastToLive, addDriver,
    removeDriver, enableBroadcast,    silenceOutput,       sessionInfos,   dropSession,         listAudioDevices,
    setAudioDevice, channelLevelsDb,  channelPitches,      inputDevices,   setInputStereo,      masterLevelDb,
};

}  // namespace

const K7MachineApi* machineApi() { return &kMachine; }

}  // namespace studio::capi
