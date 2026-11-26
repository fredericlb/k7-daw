// TransportApi.cpp — the transport area of the C ABI (k7_engine_transport.h) over EngineHost: one thin call
// per verb, the toggles resolved here. What a reader gets wrong: every function is message-thread only, which
// the Rust side's `Mt` makes a type error to break; nothing here posts.

#include "k7_engine_transport.h"

#include "CApi.h"

#include "../EngineHost.h"

#include <optional>
#include <string>
#include <vector>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }

// -1 toggles, which an unset optional asks of the host.
std::optional<bool> want(int32_t w) {
    if (w < 0) return std::nullopt;
    return w != 0;
}

void play(K7Host* h, K7Str s) { guardVoid("play", [&] { host(h)->play(str(s)); }); }
void stop(K7Host* h, K7Str s) { guardVoid("stop", [&] { host(h)->stop(str(s)); }); }
void panic(K7Host* h, K7Str s) { guardVoid("panic", [&] { host(h)->panic(str(s)); }); }
void rewind(K7Host* h, K7Str s) { guardVoid("rewind", [&] { host(h)->rewind(str(s)); }); }
void seek(K7Host* h, K7Str s, double seconds) { guardVoid("seek", [&] { host(h)->seek(str(s), seconds); }); }

void setLoop(K7Host* h, K7Str s, double start, double end, int32_t enabled) {
    guardVoid("set_loop", [&] { host(h)->setLoop(str(s), start, end, enabled != 0); });
}

int32_t isPlaying(K7Host* h, K7Str s) { return guard<int32_t>("is_playing", 0, [&] { return host(h)->isPlaying(str(s)) ? 1 : 0; }); }

void previewSample(K7Host* h, K7Str s, K7Str file, double start, double length) {
    guardVoid("preview_sample", [&] { host(h)->previewSample(str(s), str(file), start, length); });
}

void previewNote(K7Host* h, K7Str s, K7Str track, int32_t key, float velocity) {
    guardVoid("preview_note", [&] { host(h)->previewNote(str(s), str(track), key, velocity); });
}

void liveMidi(K7Host* h, K7Str s, K7Str track, const uint8_t* bytes, size_t len) {
    guardVoid("live_midi", [&] { host(h)->liveMidi(str(s), str(track), std::vector<uint8_t>(bytes, bytes + len)); });
}

int32_t setArmed(K7Host* h, K7Str s, K7Str track, int32_t w, int32_t* armed, void* ctx, K7StrEach error) {
    return guard<int32_t>("set_armed", 0, [&] {
        const auto r = host(h)->setArmed(str(s), str(track), want(w));
        if (!r.ok) {
            give(error, ctx, r.error);
            return 0;
        }
        if (armed != nullptr) *armed = r.armed ? 1 : 0;
        return 1;
    });
}

int32_t recorder(K7Host* h, K7Str s, K7Str node, int32_t clear, int32_t on, void* ctx, K7StrEach error) {
    return guard<int32_t>("recorder", 0, [&] {
        const std::string session = str(s), id = str(node);
        const auto r = clear != 0 ? host(h)->clearLastTake(session, id)
                                  : host(h)->setRecorderRecording(
                                        session, id, want(on).value_or(!host(h)->recorderRecording(session, id)));
        if (!r.ok) give(error, ctx, r.error);
        return r.ok ? 1 : 0;
    });
}

void setRecordEnabled(K7Host* h, K7Str s, int32_t w) {
    guardVoid("set_record_enabled", [&] {
        const std::string session = str(s);
        host(h)->setRecordEnabled(session, want(w).value_or(!host(h)->recordEnabled(session)));
    });
}

const K7TransportApi kTransport{
    play, stop, panic, rewind, seek, setLoop, isPlaying, previewSample, previewNote, liveMidi, setArmed, recorder,
    setRecordEnabled,
};

}  // namespace

const K7TransportApi* transportApi() { return &kTransport; }

}  // namespace studio::capi
